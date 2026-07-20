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
use pyo3::types::{PyBytes, PyString};
use serde_json::json;

use _oxyroute::microbench::{
    json_to_py, map_handler_return_status, match_route_compiled, sample_compiled_routers,
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

criterion_group!(
    benches,
    bench_match_route,
    bench_map_handler_return,
    bench_json_to_py
);
criterion_main!(benches);
