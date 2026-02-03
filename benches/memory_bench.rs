//! Memory usage benchmarks for rJMX-Exporter
//!
//! Measures heap memory allocation across different scenarios:
//! - Idle state
//! - Single scrape
//! - Concurrent scrapes

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rjmx_exporter::collector::parse_bulk_response;
use rjmx_exporter::transformer::{MetricType, PrometheusFormatter, Rule, RuleSet, TransformEngine};

/// Create test JSON for memory benchmarks
fn create_test_json(num_mbeans: usize) -> String {
    let mut mbeans = Vec::new();

    for i in 0..num_mbeans {
        mbeans.push(format!(
            r#"{{
                "request": {{"mbean": "com.example:type=Service,name=Service{}", "type": "read"}},
                "value": {{
                    "RequestCount": {},
                    "ErrorCount": {},
                    "AverageTime": {}
                }},
                "timestamp": 1609459200,
                "status": 200
            }}"#,
            i,
            i * 100,
            i * 10,
            i as f64 * 1.5
        ));
    }

    format!("[{}]", mbeans.join(","))
}

/// Create a realistic test engine with common rules
fn create_test_engine() -> TransformEngine {
    let mut ruleset = RuleSet::new();

    ruleset.add(
        Rule::builder(r"java\.lang<type=Memory><HeapMemoryUsage><(\w+)>")
            .name("jvm_memory_heap_$1_bytes")
            .metric_type(MetricType::Gauge)
            .help("JVM heap memory usage")
            .label("area", "heap")
            .build(),
    );

    ruleset.add(
        Rule::builder(r"java\.lang<type=Memory><NonHeapMemoryUsage><(\w+)>")
            .name("jvm_memory_nonheap_$1_bytes")
            .metric_type(MetricType::Gauge)
            .label("area", "nonheap")
            .build(),
    );

    ruleset.add(
        Rule::builder(r"java\.lang<type=Threading><(\w+)>")
            .name("jvm_threads_$1")
            .metric_type(MetricType::Gauge)
            .build(),
    );

    ruleset.add(
        Rule::builder(r"java\.lang<type=GarbageCollector,name=(\w+)><(\w+)>")
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

fn bench_idle_memory(c: &mut Criterion) {
    c.bench_function("memory/idle_engine_init", |b| {
        b.iter(|| {
            let engine = create_test_engine();
            std::hint::black_box(engine);
        })
    });
}

fn bench_single_scrape_memory(c: &mut Criterion) {
    let json_10 = create_test_json(10);
    let json_50 = create_test_json(50);
    let json_100 = create_test_json(100);

    let mut group = c.benchmark_group("memory/single_scrape");

    group.bench_with_input(BenchmarkId::new("mbeans", 10), &json_10, |b, json| {
        b.iter(|| {
            let responses = parse_bulk_response(json).unwrap();
            std::hint::black_box(responses);
        })
    });

    group.bench_with_input(BenchmarkId::new("mbeans", 50), &json_50, |b, json| {
        b.iter(|| {
            let responses = parse_bulk_response(json).unwrap();
            std::hint::black_box(responses);
        })
    });

    group.bench_with_input(BenchmarkId::new("mbeans", 100), &json_100, |b, json| {
        b.iter(|| {
            let responses = parse_bulk_response(json).unwrap();
            std::hint::black_box(responses);
        })
    });

    group.finish();
}

fn bench_full_pipeline_memory(c: &mut Criterion) {
    let json = create_test_json(50);
    let engine = create_test_engine();
    let formatter = PrometheusFormatter::new();

    c.bench_function("memory/full_pipeline_50_mbeans", |b| {
        b.iter(|| {
            let responses = parse_bulk_response(&json).unwrap();
            let metrics = engine.transform(&responses).unwrap_or_default();
            let output = formatter.format(&metrics);
            std::hint::black_box(output);
        })
    });
}

fn bench_large_response_memory(c: &mut Criterion) {
    let large_json = create_test_json(1000);
    let engine = create_test_engine();
    let formatter = PrometheusFormatter::new();

    c.bench_function("memory/large_response_1000_mbeans", |b| {
        b.iter(|| {
            let responses = parse_bulk_response(&large_json).unwrap();
            let metrics = engine.transform(&responses).unwrap_or_default();
            let output = formatter.format(&metrics);
            std::hint::black_box(output);
        })
    });
}

fn bench_config_memory(c: &mut Criterion) {
    let config_yaml = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
  timeout_ms: 5000

server:
  port: 9090
  path: "/metrics"

rules:
  - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
"#;

    c.bench_function("memory/config_parse", |b| {
        b.iter(|| {
            let config: rjmx_exporter::config::Config = serde_yaml::from_str(config_yaml).unwrap();
            std::hint::black_box(config);
        })
    });
}

criterion_group!(
    benches,
    bench_idle_memory,
    bench_single_scrape_memory,
    bench_full_pipeline_memory,
    bench_large_response_memory,
    bench_config_memory,
);

criterion_main!(benches);
