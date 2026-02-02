//! Startup time benchmarks for rJMX-Exporter
//!
//! Measures startup performance:
//! - Config loading time (< 10ms target)
//! - Regex compilation time (< 50ms target)
//! - Total startup time (< 100ms target)

use criterion::{criterion_group, criterion_main, BenchmarkId, Criterion};
use rjmx_exporter::config::Config;
use rjmx_exporter::transformer::{MetricType, Rule, RuleSet, TransformEngine};
use std::time::Instant;

fn generate_config_with_rules(num_rules: usize) -> String {
    let mut yaml = String::from(
        r#"
jolokia:
  url: "http://localhost:8778/jolokia"

server:
  port: 9090

rules:
"#,
    );

    for i in 0..num_rules {
        yaml.push_str(&format!(
            r#"  - pattern: "com.example<type=Service{},name=(\\w+)><(\\w+)>"
    name: "app_service{}_$2"
    type: gauge
    labels:
      service: "$1"
"#,
            i, i
        ));
    }

    yaml
}

fn bench_config_load(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup/config_load");

    for size in [5, 20, 50, 100].iter() {
        let config = generate_config_with_rules(*size);
        group.bench_with_input(BenchmarkId::new("rules", size), &config, |b, yaml| {
            b.iter(|| {
                let config: Config = serde_yaml::from_str(yaml).unwrap();
                std::hint::black_box(config);
            })
        });
    }

    group.finish();
}

fn bench_regex_compile(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup/regex_compile");

    fn create_rules(count: usize) -> Vec<Rule> {
        (0..count)
            .map(|i| {
                Rule::builder(format!(
                    r"com\.example<type=Service{},name=(\w+)><(\w+)>",
                    i
                ))
                .name(format!("app_service{}_$2", i))
                .metric_type(MetricType::Gauge)
                .label("service", "$1")
                .build()
            })
            .collect()
    }

    for count in [5, 20, 50, 100].iter() {
        let rules = create_rules(*count);
        group.bench_function(format!("rules_{}", count), |b| {
            b.iter(|| {
                let ruleset = RuleSet::from_rules(rules.clone());
                let _ = ruleset.compile_all();
                std::hint::black_box(ruleset);
            })
        });
    }

    group.finish();
}

fn bench_engine_init(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup/engine_init");

    fn create_ruleset(count: usize) -> RuleSet {
        let rules: Vec<Rule> = (0..count)
            .map(|i| {
                Rule::builder(format!(
                    r"com\.example<type=Service{},name=(\w+)><(\w+)>",
                    i
                ))
                .name(format!("app_service{}_$2", i))
                .metric_type(MetricType::Gauge)
                .build()
            })
            .collect();
        RuleSet::from_rules(rules)
    }

    for count in [20, 50].iter() {
        let ruleset = create_ruleset(*count);
        let _ = ruleset.compile_all();
        group.bench_function(format!("rules_{}", count), |b| {
            b.iter(|| {
                let engine = TransformEngine::new(ruleset.clone());
                std::hint::black_box(engine);
            })
        });
    }

    group.finish();
}

fn bench_full_startup(c: &mut Criterion) {
    let config_yaml = generate_config_with_rules(20);

    c.bench_function("startup/full_sequence", |b| {
        b.iter(|| {
            let config: Config = serde_yaml::from_str(&config_yaml).unwrap();

            let rules: Vec<Rule> = config
                .rules
                .iter()
                .map(|r| {
                    Rule::builder(&r.pattern)
                        .name(&r.name)
                        .metric_type(match r.r#type.as_str() {
                            "gauge" => MetricType::Gauge,
                            "counter" => MetricType::Counter,
                            _ => MetricType::Untyped,
                        })
                        .build()
                })
                .collect();

            let ruleset = RuleSet::from_rules(rules);
            let _ = ruleset.compile_all();
            let engine = TransformEngine::new(ruleset);
            std::hint::black_box(engine);
        })
    });
}

fn bench_regex_pattern_types(c: &mut Criterion) {
    let mut group = c.benchmark_group("startup/regex_patterns");

    group.bench_function("simple", |b| {
        b.iter(|| {
            let rule = Rule::builder(r"java\.lang<type=Memory>")
                .name("jvm_memory")
                .build();
            let _ = rule.compile();
            std::hint::black_box(rule);
        })
    });

    group.bench_function("capture_groups", |b| {
        b.iter(|| {
            let rule = Rule::builder(r"java\.lang<type=(\w+)><(\w+)><(\w+)>")
                .name("jvm_$1_$2_$3")
                .build();
            let _ = rule.compile();
            std::hint::black_box(rule);
        })
    });

    group.bench_function("named_groups", |b| {
        b.iter(|| {
            let rule =
                Rule::builder(r"java\.lang<type=(?P<type>\w+)><(?P<attr>\w+)><(?P<key>\w+)>")
                    .name("jvm_${type}_${attr}_${key}")
                    .build();
            let _ = rule.compile();
            std::hint::black_box(rule);
        })
    });

    group.finish();
}

fn bench_startup_target(c: &mut Criterion) {
    let config_yaml = generate_config_with_rules(50);

    c.bench_function("startup/target_verification", |b| {
        b.iter_custom(|iters| {
            let mut total = std::time::Duration::ZERO;

            for _ in 0..iters {
                let start = Instant::now();

                let config: Config = serde_yaml::from_str(&config_yaml).unwrap();
                let rules: Vec<Rule> = config
                    .rules
                    .iter()
                    .map(|r| {
                        Rule::builder(&r.pattern)
                            .name(&r.name)
                            .metric_type(MetricType::Gauge)
                            .build()
                    })
                    .collect();

                let ruleset = RuleSet::from_rules(rules);
                let _ = ruleset.compile_all();
                let engine = TransformEngine::new(ruleset);
                std::hint::black_box(&engine);

                total += start.elapsed();
            }

            total
        })
    });
}

criterion_group!(
    benches,
    bench_config_load,
    bench_regex_compile,
    bench_engine_init,
    bench_full_startup,
    bench_regex_pattern_types,
    bench_startup_target,
);

criterion_main!(benches);
