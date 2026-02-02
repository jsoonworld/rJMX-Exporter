//! Collector 벤치마크
//!
//! JSON 파싱 성능 측정

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rjmx_exporter::collector::parse_response;

fn benchmark_parse_response(c: &mut Criterion) {
    let simple_json = r#"{
        "request": {"mbean": "java.lang:type=Threading", "type": "read"},
        "value": 42,
        "timestamp": 1609459200,
        "status": 200
    }"#;

    let composite_json = r#"{
        "request": {"mbean": "java.lang:type=Memory", "type": "read"},
        "value": {
            "init": 268435456,
            "committed": 268435456,
            "max": 4294967296,
            "used": 52428800
        },
        "timestamp": 1609459200,
        "status": 200
    }"#;

    let wildcard_json = r#"{
        "request": {"mbean": "java.lang:type=GarbageCollector,name=*", "type": "read"},
        "value": {
            "java.lang:type=GarbageCollector,name=G1 Young Generation": {
                "CollectionCount": 42,
                "CollectionTime": 1234
            },
            "java.lang:type=GarbageCollector,name=G1 Old Generation": {
                "CollectionCount": 5,
                "CollectionTime": 567
            }
        },
        "timestamp": 1609459200,
        "status": 200
    }"#;

    let mut group = c.benchmark_group("parse_response");

    group.bench_with_input(
        BenchmarkId::new("simple", "number"),
        &simple_json,
        |b, json| b.iter(|| parse_response(json)),
    );

    group.bench_with_input(
        BenchmarkId::new("composite", "memory"),
        &composite_json,
        |b, json| b.iter(|| parse_response(json)),
    );

    group.bench_with_input(
        BenchmarkId::new("wildcard", "gc"),
        &wildcard_json,
        |b, json| b.iter(|| parse_response(json)),
    );

    group.finish();
}

criterion_group!(benches, benchmark_parse_response);
criterion_main!(benches);
