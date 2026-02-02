//! Scrape latency benchmarks for rJMX-Exporter
//!
//! Measures scrape pipeline performance:
//! - JSON parsing (< 2ms target)
//! - Transform processing (< 2ms target)
//! - Full scrape latency (< 10ms target)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion, Throughput};
use rjmx_exporter::collector::{parse_bulk_response, parse_response};
use rjmx_exporter::transformer::{MetricType, PrometheusFormatter, Rule, RuleSet, TransformEngine};

const SIMPLE_JSON: &str = r#"{
    "request": {"mbean": "java.lang:type=Threading", "attribute": "ThreadCount", "type": "read"},
    "value": 42,
    "timestamp": 1609459200,
    "status": 200
}"#;

const COMPOSITE_JSON: &str = r#"{
    "request": {"mbean": "java.lang:type=Memory", "attribute": "HeapMemoryUsage", "type": "read"},
    "value": {
        "init": 268435456,
        "committed": 536870912,
        "max": 4294967296,
        "used": 134217728
    },
    "timestamp": 1609459200,
    "status": 200
}"#;

const WILDCARD_JSON: &str = r#"{
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

fn generate_bulk_response(num_mbeans: usize) -> String {
    let responses: Vec<String> = (0..num_mbeans)
        .map(|i| {
            format!(
                r#"{{
                    "request": {{"mbean": "com.example:type=Service,name=Service{}", "type": "read"}},
                    "value": {{
                        "RequestCount": {},
                        "ErrorCount": {},
                        "AverageResponseTime": {}
                    }},
                    "timestamp": 1609459200,
                    "status": 200
                }}"#,
                i,
                i * 1000 + 100,
                i * 10,
                i as f64 * 1.5 + 10.0
            )
        })
        .collect();

    format!("[{}]", responses.join(","))
}

fn create_test_engine() -> TransformEngine {
    let mut ruleset = RuleSet::new();

    ruleset.add(
        Rule::builder(r"java\.lang<type=Memory><HeapMemoryUsage><(\w+)>")
            .name("jvm_memory_heap_$1_bytes")
            .metric_type(MetricType::Gauge)
            .label("area", "heap")
            .build(),
    );

    ruleset.add(
        Rule::builder(r"java\.lang<type=Threading><(\w+)>")
            .name("jvm_threads_$1")
            .metric_type(MetricType::Gauge)
            .build(),
    );

    ruleset.add(
        Rule::builder(r"java\.lang<type=GarbageCollector,name=([^>]+)><(\w+)>")
            .name("jvm_gc_$2")
            .metric_type(MetricType::Counter)
            .label("gc", "$1")
            .build(),
    );

    ruleset.add(
        Rule::builder(r"com\.example<type=Service,name=(\w+)><(\w+)>")
            .name("app_service_$2")
            .metric_type(MetricType::Gauge)
            .label("service", "$1")
            .build(),
    );

    let _ = ruleset.compile_all();
    TransformEngine::new(ruleset)
}

fn bench_json_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrape/json_parsing");

    group.bench_function("simple", |b| {
        b.iter(|| {
            let result = parse_response(SIMPLE_JSON).unwrap();
            std::hint::black_box(result);
        })
    });

    group.bench_function("composite", |b| {
        b.iter(|| {
            let result = parse_response(COMPOSITE_JSON).unwrap();
            std::hint::black_box(result);
        })
    });

    group.bench_function("wildcard", |b| {
        b.iter(|| {
            let result = parse_response(WILDCARD_JSON).unwrap();
            std::hint::black_box(result);
        })
    });

    group.finish();
}

fn bench_bulk_json_parsing(c: &mut Criterion) {
    let mut group = c.benchmark_group("scrape/bulk_json_parsing");

    for size in [10, 50, 100, 500].iter() {
        let json = generate_bulk_response(*size);
        let json_bytes = json.len();

        group.throughput(Throughput::Bytes(json_bytes as u64));
        group.bench_with_input(BenchmarkId::new("mbeans", size), &json, |b, json| {
            b.iter(|| {
                let result = parse_bulk_response(json).unwrap();
                std::hint::black_box(result);
            })
        });
    }

    group.finish();
}

fn bench_transform_processing(c: &mut Criterion) {
    let engine = create_test_engine();
    let mut group = c.benchmark_group("scrape/transform");

    group.bench_function("simple_numeric", |b| {
        let responses = vec![parse_response(SIMPLE_JSON).unwrap()];
        b.iter(|| {
            let metrics = engine.transform(&responses).unwrap_or_default();
            std::hint::black_box(metrics);
        })
    });

    group.bench_function("composite", |b| {
        let responses = vec![parse_response(COMPOSITE_JSON).unwrap()];
        b.iter(|| {
            let metrics = engine.transform(&responses).unwrap_or_default();
            std::hint::black_box(metrics);
        })
    });

    group.bench_function("wildcard", |b| {
        let responses = vec![parse_response(WILDCARD_JSON).unwrap()];
        b.iter(|| {
            let metrics = engine.transform(&responses).unwrap_or_default();
            std::hint::black_box(metrics);
        })
    });

    group.finish();
}

fn bench_bulk_transform(c: &mut Criterion) {
    let engine = create_test_engine();
    let mut group = c.benchmark_group("scrape/bulk_transform");

    for size in [10, 50, 100, 500].iter() {
        let json = generate_bulk_response(*size);
        let responses = parse_bulk_response(&json).unwrap();

        group.throughput(Throughput::Elements(*size as u64));
        group.bench_with_input(
            BenchmarkId::new("mbeans", size),
            &responses,
            |b, responses| {
                b.iter(|| {
                    let metrics = engine.transform(responses).unwrap_or_default();
                    std::hint::black_box(metrics);
                })
            },
        );
    }

    group.finish();
}

fn bench_prometheus_formatting(c: &mut Criterion) {
    let engine = create_test_engine();
    let formatter = PrometheusFormatter::new();
    let mut group = c.benchmark_group("scrape/formatting");

    for count in [10, 50, 100, 500].iter() {
        let json = generate_bulk_response(*count);
        let responses = parse_bulk_response(&json).unwrap();
        let metrics = engine.transform(&responses).unwrap_or_default();

        group.throughput(Throughput::Elements(*count as u64));
        group.bench_with_input(
            BenchmarkId::new("metrics", count),
            &metrics,
            |b, metrics| {
                b.iter(|| {
                    let output = formatter.format(metrics);
                    std::hint::black_box(output);
                })
            },
        );
    }

    group.finish();
}

fn bench_full_scrape_pipeline(c: &mut Criterion) {
    let engine = create_test_engine();
    let formatter = PrometheusFormatter::new();
    let mut group = c.benchmark_group("scrape/full_pipeline");

    let typical_json = generate_bulk_response(50);
    group.bench_function("typical_50_mbeans", |b| {
        b.iter(|| {
            let responses = parse_bulk_response(&typical_json).unwrap();
            let metrics = engine.transform(&responses).unwrap_or_default();
            let output = formatter.format(&metrics);
            std::hint::black_box(output);
        })
    });

    let large_json = generate_bulk_response(200);
    group.bench_function("large_200_mbeans", |b| {
        b.iter(|| {
            let responses = parse_bulk_response(&large_json).unwrap();
            let metrics = engine.transform(&responses).unwrap_or_default();
            let output = formatter.format(&metrics);
            std::hint::black_box(output);
        })
    });

    let xlarge_json = generate_bulk_response(1000);
    group.bench_function("xlarge_1000_mbeans", |b| {
        b.iter(|| {
            let responses = parse_bulk_response(&xlarge_json).unwrap();
            let metrics = engine.transform(&responses).unwrap_or_default();
            let output = formatter.format(&metrics);
            std::hint::black_box(output);
        })
    });

    group.finish();
}

fn bench_latency_target(c: &mut Criterion) {
    let engine = create_test_engine();
    let formatter = PrometheusFormatter::new();
    let json = generate_bulk_response(100);

    c.bench_function("scrape/target_verification_100_mbeans", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let start = std::time::Instant::now();
                let responses = parse_bulk_response(&json).unwrap();
                let metrics = engine.transform(&responses).unwrap_or_default();
                let output = formatter.format(&metrics);
                std::hint::black_box(&output);
                total += start.elapsed();
            }

            total
        })
    });
}

criterion_group!(
    benches,
    bench_json_parsing,
    bench_bulk_json_parsing,
    bench_transform_processing,
    bench_bulk_transform,
    bench_prometheus_formatting,
    bench_full_scrape_pipeline,
    bench_latency_target,
);

criterion_main!(benches);
