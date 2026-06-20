use pyo3::prelude::*;
use pyo3::types::{PyBool, PyDict, PyFloat, PyInt, PyList, PyString, PyTuple};
use sqlx::Row;

#[pyclass(module = "oxyroute._oxyroute")]
#[derive(Clone)]
pub struct DBQuery {
    pub query: String,
    pub args: PyObject,
}

#[pymethods]
impl DBQuery {
    #[new]
    #[pyo3(signature = (query, args=None))]
    fn new(py: Python<'_>, query: String, args: Option<Py<PyAny>>) -> PyResult<Self> {
        let args = if let Some(a) = args {
            if let Ok(tup) = a.downcast_bound::<PyTuple>(py) {
                tup.clone().unbind().into()
            } else if let Ok(lst) = a.downcast_bound::<PyList>(py) {
                PyTuple::new(py, lst.iter())?.unbind().into()
            } else {
                return Err(pyo3::exceptions::PyTypeError::new_err(
                    "args must be a list or tuple",
                ));
            }
        } else {
            PyTuple::empty(py).unbind().into()
        };
        Ok(Self { query, args })
    }
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

    let rows = match q.fetch_all(pool).await {
        Ok(r) => r,
        Err(e) => {
            return Err(pyo3::exceptions::PyRuntimeError::new_err(format!(
                "DBQuery failed: {}",
                e
            )));
        }
    };

    Python::with_gil(|py| -> PyResult<PyObject> {
        let out = PyList::empty(py);
        for row in rows {
            let d = PyDict::new(py);
            for (i, col) in row.columns().iter().enumerate() {
                use sqlx::{Column, TypeInfo, ValueRef};
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
                        let v: String =
                            sqlx::Decode::<'_, sqlx::Postgres>::decode(val_ref).unwrap();
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
            out.append(d)?;
        }
        Ok(out.into())
    })
}
