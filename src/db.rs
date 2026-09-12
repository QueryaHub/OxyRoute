use futures_util::StreamExt;
use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use sqlx::Row;

/// Represents a parameterized database query executed natively by sqlx in Rust.
///
/// **SECURITY NOTE**: Always use positional parameter placeholders (`$1`, `$2`, ...)
/// and pass parameters in `args`. **NEVER** use Python f-strings or string concatenation
/// to construct SQL queries, as this introduces SQL injection vulnerabilities.
///
/// ### Positive Example (Safe):
/// ```python
/// query = DBQuery("SELECT id, name FROM users WHERE email = $1 AND active = $2", (email, True))
/// ```
///
/// ### Chunked / Streaming Example (Memory Bounded):
/// ```python
/// query = DBQuery("SELECT * FROM large_table", chunk_size=100)
/// ```
#[pyclass(module = "oxyroute._oxyroute")]
#[derive(Clone)]
pub struct DBQuery {
    #[pyo3(get)]
    pub query: String,
    #[pyo3(get)]
    pub args: PyObject,
    #[pyo3(get)]
    pub chunk_size: Option<usize>,
}

#[pymethods]
impl DBQuery {
    /// Construct a new parameterized database query.
    ///
    /// Parameters:
    /// - `query`: Parameterized SQL string using `$1`, `$2`, ... positional placeholders.
    /// - `args`: Tuple or list of argument values to bind safely.
    /// - `chunk_size`: Optional chunk size for batched row decoding.
    #[new]
    #[pyo3(signature = (query, args=None, chunk_size=None))]
    fn new(
        py: Python<'_>,
        query: String,
        args: Option<Py<PyAny>>,
        chunk_size: Option<usize>,
    ) -> PyResult<Self> {
        let args = if let Some(a) = args {
            if let Ok(tup) = a.downcast_bound::<PyTuple>(py) {
                tup.clone().unbind().into()
            } else if let Ok(lst) = a.downcast_bound::<PyList>(py) {
                PyTuple::new(py, lst.iter())?.unbind().into()
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "args must be a list or tuple of query parameters",
                ));
            }
        } else {
            PyTuple::empty(py).unbind().into()
        };

        if let Some(cs) = chunk_size {
            if cs == 0 {
                return Err(pyo3::exceptions::PyValueError::new_err(
                    "chunk_size must be greater than 0",
                ));
            }
        }

        Ok(Self {
            query,
            args,
            chunk_size,
        })
    }
}

fn decode_pg_row_to_dict<'py>(
    py: Python<'py>,
    row: &sqlx::postgres::PgRow,
) -> PyResult<Bound<'py, PyDict>> {
    use sqlx::{Column, TypeInfo, ValueRef};
    let d = PyDict::new(py);
    for (i, col) in row.columns().iter().enumerate() {
        let name = col.name();
        let val_ref = row.try_get_raw(i).unwrap();
        if val_ref.is_null() {
            d.set_item(name, py.None())?;
            continue;
        }
        let info = val_ref.type_info();
        let ty = info.name();
        match ty {
            "BOOL" => {
                let v: bool = sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
                d.set_item(name, v)?;
            }
            "INT2" => {
                let v: i16 = sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
                d.set_item(name, v)?;
            }
            "INT4" => {
                let v: i32 = sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
                d.set_item(name, v)?;
            }
            "INT8" => {
                let v: i64 = sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
                d.set_item(name, v)?;
            }
            "FLOAT4" => {
                let v: f32 = sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
                d.set_item(name, v)?;
            }
            "FLOAT8" => {
                let v: f64 = sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
                d.set_item(name, v)?;
            }
            "TEXT" | "VARCHAR" | "CHAR" | "\"CHAR\"" | "NAME" => {
                let v: String = sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
                d.set_item(name, v)?;
            }
            "JSON" | "JSONB" => {
                let v: serde_json::Value =
                    sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
                let py_v = crate::schema::json_to_py(py, &v)?;
                d.set_item(name, py_v)?;
            }
            _ => {
                // Fallback: try as string
                if let Ok(v) = sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref) {
                    let s: String = v;
                    d.set_item(name, s)?;
                } else {
                    d.set_item(name, py.None())?;
                }
            }
        }
    }
    Ok(d)
}

pub async fn execute_query(pool: &sqlx::PgPool, db_query: &DBQuery) -> PyResult<PyObject> {
    let q = sqlx::query(sqlx::AssertSqlSafe(db_query.query.as_str()));
    let q = Python::with_gil(|py| -> PyResult<_> {
        let mut q = q;
        let args_tuple = db_query.args.bind(py).downcast::<PyTuple>().unwrap();

        for arg in args_tuple.iter() {
            if arg.is_none() {
                let opt: Option<String> = None;
                q = q.bind(opt);
            } else if let Ok(b) = arg.downcast::<PyBool>() {
                q = q.bind(b.is_true());
            } else if let Ok(i) = arg.downcast::<PyInt>() {
                q = q.bind(i.extract::<i64>()?);
            } else if let Ok(f) = arg.downcast::<PyFloat>() {
                q = q.bind(f.extract::<f64>()?);
            } else if let Ok(s) = arg.downcast::<PyString>() {
                // We need to own the string since q will be moved outside
                q = q.bind(s.to_str()?.to_string());
            } else {
                return Err(pyo3::exceptions::PyValueError::new_err(format!(
                    "Unsupported DBQuery argument type: {}",
                    arg.get_type()
                )));
            }
        }
        Ok(q)
    })?;

    let mut stream = q.fetch(pool);

    if let Some(chunk_sz) = db_query.chunk_size {
        let chunks = Python::with_gil(|py| PyList::empty(py).unbind());
        let mut current_chunk: Vec<PyObject> = Vec::with_capacity(chunk_sz);

        while let Some(row_res) = stream.next().await {
            let row = match row_res {
                Ok(r) => r,
                Err(e) => {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                        "DBQuery streaming failed: {e}"
                    )));
                }
            };
            let dict_obj = Python::with_gil(|py| -> PyResult<PyObject> {
                let d = decode_pg_row_to_dict(py, &row)?;
                Ok(d.unbind().into())
            })?;
            current_chunk.push(dict_obj);
            if current_chunk.len() >= chunk_sz {
                Python::with_gil(|py| -> PyResult<()> {
                    let chunk_list = PyList::new(py, current_chunk.drain(..))?;
                    chunks.bind(py).append(chunk_list)?;
                    Ok(())
                })?;
            }
        }
        if !current_chunk.is_empty() {
            Python::with_gil(|py| -> PyResult<()> {
                let chunk_list = PyList::new(py, current_chunk.drain(..))?;
                chunks.bind(py).append(chunk_list)?;
                Ok(())
            })?;
        }
        Ok(Python::with_gil(|py| {
            chunks.into_bound(py).into_any().unbind()
        }))
    } else {
        let out = Python::with_gil(|py| PyList::empty(py).unbind());
        while let Some(row_res) = stream.next().await {
            let row = match row_res {
                Ok(r) => r,
                Err(e) => {
                    return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                        "DBQuery streaming failed: {e}"
                    )));
                }
            };
            Python::with_gil(|py| -> PyResult<()> {
                let d = decode_pg_row_to_dict(py, &row)?;
                out.bind(py).append(d)?;
                Ok(())
            })?;
        }
        Ok(Python::with_gil(|py| {
            out.into_bound(py).into_any().unbind()
        }))
    }
}
