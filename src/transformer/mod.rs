//! Metric transformation module
//!
//! This module handles transformation of JMX metrics
//! to Prometheus format based on configured rules.
//!
//! # Overview
//!
//! The transformer module provides rule-based metric transformation:
//!
//! - **Rules**: Define patterns to match MBean names and transform them
//! - **MetricType**: Prometheus metric types (gauge, counter, untyped)
//! - **RuleSet**: Collection of rules with batch operations
//! - **TransformEngine**: Applies rules to convert MBean data to metrics
//! - **PrometheusFormatter**: Formats metrics into Prometheus text format
//!
//! # Example
//!
//! ```ignore
//! use rjmx_exporter::transformer::{
//!     Rule, RuleSet, MetricType, TransformEngine, PrometheusFormatter
//! };
//!
//! // Create rules
//! let ruleset = RuleSet::from_rules(vec![
//!     Rule::builder(r"java\.lang<type=Memory><HeapMemoryUsage>(\w+)")
//!         .name("jvm_memory_heap_$1_bytes")
//!         .metric_type(MetricType::Gauge)
//!         .help("JVM heap memory usage")
//!         .build(),
//! ]);
//!
//! // Create transform engine
//! let engine = TransformEngine::new(ruleset);
//!
//! // Transform Jolokia responses to Prometheus metrics
//! let metrics = engine.transform(&responses)?;
//!
//! // Format output
//! let formatter = PrometheusFormatter::new();
//! let output = formatter.format(&metrics);
//! ```

pub mod engine;
pub mod formatter;
pub mod rules;

pub use engine::{PrometheusMetric, TransformEngine};
pub use formatter::PrometheusFormatter;
pub use rules::{
    convert_java_regex, MetricType, Rule, RuleBuilder, RuleError, RuleMatch, RuleResult, RuleSet,
};

/// Legacy transformer alias for backwards compatibility
#[deprecated(since = "0.2.0", note = "Use TransformEngine instead")]
pub type Transformer = TransformEngine;
