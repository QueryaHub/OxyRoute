//! Criterion microbenchmarks for hot-path primitives (issue #110).
//!
//! Run from the repo root (requires a linked Python interpreter via PyO3)::
//!
//! ```bash
//! cargo bench --bench hot_path
//! ```
//!
//! Not part of default CI. Collect a baseline before perf PRs and attach Criterion
//! HTML reports (`target/criterion/`) or key numbers in the PR description.

use std::hint::black_box;

use criterion::{criterion_group, criterion_main, Criterion};
use pyo3::prelude::*;
use pyo3::types::{PyBytes, PyDict, PyList, PyString};
use serde_json::json;

use _oxyroute::microbench::{
    header_get_lax, json_to_py, map_handler_return_status, match_route_compiled, py_to_json_bytes,
    sample_compiled_routers,
};

fn bench_match_route(c: &mut Criterion) {
    let compiled = sample_compiled_routers();
    let mut group = c.benchmark_group("match_route_compiled");
    group.bench_function("static", |b| {
        b.iter(|| {
            let hit = match_route_compiled(black_box(&compiled), "GET", black_box("/hello"));
            black_box(hit)
        })
    });
    group.bench_function("param", |b| {
        b.iter(|| {
            let hit = match_route_compiled(black_box(&compiled), "GET", black_box("/items/42"));
            black_box(hit)
        })
    });
    group.finish();
}

fn bench_map_handler_return(c: &mut Criterion) {
    Python::with_gil(|py| {
        let s = PyString::new(py, "hello world");
        let buf = PyBytes::new(py, b"hello world");
        let d = PyDict::new(py);
        d.set_item("message", "hello world").unwrap();
        d.set_item("count", 42).unwrap();
        let l = PyList::new(py, ["a", "b", "c"]).unwrap();
        let mut group = c.benchmark_group("map_handler_return");
        group.bench_function("str", |b| {
            b.iter(|| {
                let status = map_handler_return_status(py, black_box(s.as_any())).unwrap();
                black_box(status)
            })
        });
        group.bench_function("bytes", |b| {
            b.iter(|| {
                let status = map_handler_return_status(py, black_box(buf.as_any())).unwrap();
                black_box(status)
            })
        });
        group.bench_function("dict", |b| {
            b.iter(|| {
                let status = map_handler_return_status(py, black_box(d.as_any())).unwrap();
                black_box(status)
            })
        });
        group.bench_function("list", |b| {
            b.iter(|| {
                let status = map_handler_return_status(py, black_box(l.as_any())).unwrap();
                black_box(status)
            })
        });
        group.finish();
    });
}

fn bench_json_to_py(c: &mut Criterion) {
    let small = json!({"a": 1, "b": "x", "c": true});
    let nested = json!({
        "items": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}],
        "meta": {"ok": true, "n": 2}
    });
    Python::with_gil(|py| {
        let mut group = c.benchmark_group("json_to_py");
        group.bench_function("small_object", |b| {
            b.iter(|| {
                let obj = json_to_py(py, black_box(&small)).unwrap();
                black_box(obj)
            })
        });
        group.bench_function("nested", |b| {
            b.iter(|| {
                let obj = json_to_py(py, black_box(&nested)).unwrap();
                black_box(obj)
            })
        });
        group.finish();
    });
}

fn bench_json_scale(c: &mut Criterion) {
    let item = json!({"id": 12345, "name": "Product item name", "price": 99.95, "active": true});
    let generate_payload = |count: usize| -> serde_json::Value {
        let items: Vec<serde_json::Value> = (0..count).map(|_| item.clone()).collect();
        json!({ "items": items, "count": count })
    };

    let p_1kb = generate_payload(15);
    let p_10kb = generate_payload(150);
    let p_100kb = generate_payload(1500);

    Python::with_gil(|py| {
        let mut group = c.benchmark_group("json_scale");
        group.bench_function("1kb", |b| {
            b.iter(|| {
                let obj = json_to_py(py, black_box(&p_1kb)).unwrap();
                black_box(obj)
            })
        });
        group.bench_function("10kb", |b| {
            b.iter(|| {
                let obj = json_to_py(py, black_box(&p_10kb)).unwrap();
                black_box(obj)
            })
        });
        group.bench_function("100kb", |b| {
            b.iter(|| {
                let obj = json_to_py(py, black_box(&p_100kb)).unwrap();
                black_box(obj)
            })
        });
        group.finish();
    });
}

fn bench_py_to_json(c: &mut Criterion) {
    Python::with_gil(|py| {
        let small = json!({"a": 1, "b": "x", "c": true});
        let py_small = json_to_py(py, &small).unwrap();
        let nested = json!({
            "items": [{"id": 1, "name": "a"}, {"id": 2, "name": "b"}],
            "meta": {"ok": true, "n": 2}
        });
        let py_nested = json_to_py(py, &nested).unwrap();
        let mut group = c.benchmark_group("py_to_json");
        group.bench_function("small_object", |b| {
            b.iter(|| {
                let bytes = py_to_json_bytes(py, black_box(py_small.bind(py))).unwrap();
                black_box(bytes)
            })
        });
        group.bench_function("nested", |b| {
            b.iter(|| {
                let bytes = py_to_json_bytes(py, black_box(py_nested.bind(py))).unwrap();
                black_box(bytes)
            })
        });
        group.finish();
    });
}

fn bench_header_get_lax(c: &mut Criterion) {
    Python::with_gil(|py| {
        let d = PyDict::new(py);
        d.set_item("authorization", "Bearer token123").unwrap();
        d.set_item("content-type", "application/json").unwrap();
        d.set_item("host", "example.com").unwrap();
        d.set_item("user-agent", "bench/1.0").unwrap();
        let bound = d.as_any();

        let mut group = c.benchmark_group("header_get_lax");
        group.bench_function("hit_exact", |b| {
            b.iter(|| {
                let val = header_get_lax(black_box(bound), black_box("authorization"));
                black_box(val)
            })
        });
        group.bench_function("miss", |b| {
            b.iter(|| {
                let val = header_get_lax(black_box(bound), black_box("cookie"));
                black_box(val)
            })
        });
        group.finish();
    });
}

criterion_group!(
    benches,
    bench_match_route,
    bench_map_handler_return,
    bench_json_to_py,
    bench_py_to_json,
    bench_json_scale,
    bench_header_get_lax
);
criterion_main!(benches);
