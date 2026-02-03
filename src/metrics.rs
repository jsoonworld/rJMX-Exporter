//! Internal observability metrics for rJMX-Exporter
//!
//! This module provides detailed metrics about the exporter's own operation
//! for better observability and debugging.
//!
//! # Metrics
//!
//! ## Per-target metrics
//! - `rjmx_scrape_success_total{target="..."}` - Counter of successful scrapes
//! - `rjmx_scrape_failure_total{target="..."}` - Counter of failed scrapes
//! - `rjmx_scrape_duration_seconds{target="..."}` - Histogram of scrape durations
//!
//! ## Per-rule metrics
//! - `rjmx_rule_matches_total{rule="..."}` - Counter of rule matches
//! - `rjmx_rule_errors_total{rule="..."}` - Counter of rule errors
//!
//! ## Connection pool metrics
//! - `rjmx_http_connections_active` - Gauge of active HTTP connections
//! - `rjmx_http_connections_idle` - Gauge of idle HTTP connections
//!
//! ## Config metrics
//! - `rjmx_config_reload_total` - Counter of config reloads
//! - `rjmx_config_last_reload_timestamp` - Timestamp of last config reload

use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

use crate::transformer::{MetricType, PrometheusMetric};

/// Default histogram buckets for scrape duration (in seconds)
/// Aligned with Prometheus conventions for HTTP request durations
pub const DEFAULT_HISTOGRAM_BUCKETS: &[f64] = &[
    0.005, 0.01, 0.025, 0.05, 0.1, 0.25, 0.5, 1.0, 2.5, 5.0, 10.0,
];

/// Thread-safe counter using atomic operations
#[derive(Debug, Default)]
pub struct Counter {
    value: AtomicU64,
}

impl Counter {
    /// Create a new counter initialized to 0
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0),
        }
    }

    /// Increment the counter by 1
    pub fn inc(&self) {
        self.value.fetch_add(1, Ordering::Relaxed);
    }

    /// Increment the counter by a specific amount
    pub fn inc_by(&self, n: u64) {
        self.value.fetch_add(n, Ordering::Relaxed);
    }

    /// Get the current value
    pub fn get(&self) -> u64 {
        self.value.load(Ordering::Relaxed)
    }

    /// Reset the counter to 0
    pub fn reset(&self) {
        self.value.store(0, Ordering::Relaxed);
    }
}

impl Clone for Counter {
    fn clone(&self) -> Self {
        Self {
            value: AtomicU64::new(self.get()),
        }
    }
}

/// Thread-safe gauge using atomic operations
#[derive(Debug, Default)]
pub struct Gauge {
    /// Stored as bits of f64 for atomic operations
    value: AtomicU64,
}

impl Gauge {
    /// Create a new gauge initialized to 0
    pub fn new() -> Self {
        Self {
            value: AtomicU64::new(0.0_f64.to_bits()),
        }
    }

    /// Set the gauge to a specific value
    pub fn set(&self, v: f64) {
        self.value.store(v.to_bits(), Ordering::Relaxed);
    }

    /// Get the current value
    pub fn get(&self) -> f64 {
        f64::from_bits(self.value.load(Ordering::Relaxed))
    }

    /// Increment the gauge by a specific amount
    pub fn inc(&self, v: f64) {
        loop {
            let current = self.value.load(Ordering::Relaxed);
            let new = f64::from_bits(current) + v;
            if self
                .value
                .compare_exchange_weak(current, new.to_bits(), Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }
    }

    /// Decrement the gauge by a specific amount
    pub fn dec(&self, v: f64) {
        self.inc(-v);
    }

    /// Set the gauge to the current Unix timestamp
    pub fn set_to_current_time(&self) {
        let timestamp = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map(|d| d.as_secs_f64())
            .unwrap_or(0.0);
        self.set(timestamp);
    }
}

impl Clone for Gauge {
    fn clone(&self) -> Self {
        Self {
            value: AtomicU64::new(self.value.load(Ordering::Relaxed)),
        }
    }
}

/// Thread-safe histogram for measuring distributions
#[derive(Debug)]
pub struct Histogram {
    /// Bucket boundaries (upper bounds)
    buckets: Vec<f64>,
    /// Bucket counters (count of observations <= bucket boundary)
    bucket_counts: Vec<AtomicU64>,
    /// Sum of all observed values
    sum: AtomicU64,
    /// Total count of observations
    count: AtomicU64,
}

impl Histogram {
    /// Create a new histogram with the given bucket boundaries
    pub fn new(buckets: &[f64]) -> Self {
        let mut sorted_buckets: Vec<f64> = buckets.to_vec();
        sorted_buckets.sort_by(|a, b| a.partial_cmp(b).unwrap_or(std::cmp::Ordering::Equal));

        // Add +Inf bucket if not present
        if sorted_buckets
            .last()
            .map(|v| !v.is_infinite())
            .unwrap_or(true)
        {
            sorted_buckets.push(f64::INFINITY);
        }

        let bucket_counts = (0..sorted_buckets.len())
            .map(|_| AtomicU64::new(0))
            .collect();

        Self {
            buckets: sorted_buckets,
            bucket_counts,
            sum: AtomicU64::new(0.0_f64.to_bits()),
            count: AtomicU64::new(0),
        }
    }

    /// Create a histogram with default buckets for scrape durations
    pub fn with_default_buckets() -> Self {
        Self::new(DEFAULT_HISTOGRAM_BUCKETS)
    }

    /// Observe a value
    pub fn observe(&self, v: f64) {
        // Update count
        self.count.fetch_add(1, Ordering::Relaxed);

        // Update sum (atomic f64 add)
        loop {
            let current = self.sum.load(Ordering::Relaxed);
            let new = f64::from_bits(current) + v;
            if self
                .sum
                .compare_exchange_weak(current, new.to_bits(), Ordering::Relaxed, Ordering::Relaxed)
                .is_ok()
            {
                break;
            }
        }

        // Update bucket counts
        for (i, &bound) in self.buckets.iter().enumerate() {
            if v <= bound {
                self.bucket_counts[i].fetch_add(1, Ordering::Relaxed);
            }
        }
    }

    /// Get the sum of all observations
    pub fn get_sum(&self) -> f64 {
        f64::from_bits(self.sum.load(Ordering::Relaxed))
    }

    /// Get the total count of observations
    pub fn get_count(&self) -> u64 {
        self.count.load(Ordering::Relaxed)
    }

    /// Get bucket boundaries and their cumulative counts
    pub fn get_buckets(&self) -> Vec<(f64, u64)> {
        self.buckets
            .iter()
            .zip(self.bucket_counts.iter())
            .map(|(&bound, count)| (bound, count.load(Ordering::Relaxed)))
            .collect()
    }
}

impl Clone for Histogram {
    fn clone(&self) -> Self {
        Self {
            buckets: self.buckets.clone(),
            bucket_counts: self
                .bucket_counts
                .iter()
                .map(|c| AtomicU64::new(c.load(Ordering::Relaxed)))
                .collect(),
            sum: AtomicU64::new(self.sum.load(Ordering::Relaxed)),
            count: AtomicU64::new(self.count.load(Ordering::Relaxed)),
        }
    }
}

impl Default for Histogram {
    fn default() -> Self {
        Self::with_default_buckets()
    }
}

/// Per-target metrics
#[derive(Debug, Clone)]
pub struct TargetMetrics {
    /// Counter of successful scrapes
    pub scrape_success_total: Counter,
    /// Counter of failed scrapes
    pub scrape_failure_total: Counter,
    /// Histogram of scrape durations
    pub scrape_duration_seconds: Histogram,
}

impl Default for TargetMetrics {
    fn default() -> Self {
        Self {
            scrape_success_total: Counter::new(),
            scrape_failure_total: Counter::new(),
            scrape_duration_seconds: Histogram::with_default_buckets(),
        }
    }
}

/// Per-rule metrics
#[derive(Debug, Clone, Default)]
pub struct RuleMetrics {
    /// Counter of rule matches
    pub matches_total: Counter,
    /// Counter of rule errors
    pub errors_total: Counter,
}

/// Connection pool metrics
#[derive(Debug, Clone, Default)]
pub struct ConnectionPoolMetrics {
    /// Gauge of active HTTP connections
    pub active: Gauge,
    /// Gauge of idle HTTP connections
    pub idle: Gauge,
}

/// Config metrics
#[derive(Debug, Clone, Default)]
pub struct ConfigMetrics {
    /// Counter of config reloads
    pub reload_total: Counter,
    /// Timestamp of last config reload
    pub last_reload_timestamp: Gauge,
}

/// Internal metrics registry
///
/// Thread-safe registry for all internal observability metrics.
#[derive(Debug, Clone)]
pub struct InternalMetrics {
    /// Per-target metrics, keyed by target name/URL
    targets: Arc<RwLock<HashMap<String, TargetMetrics>>>,
    /// Per-rule metrics, keyed by rule pattern
    rules: Arc<RwLock<HashMap<String, RuleMetrics>>>,
    /// Connection pool metrics
    pub connections: Arc<ConnectionPoolMetrics>,
    /// Config metrics
    pub config: Arc<ConfigMetrics>,
}

impl Default for InternalMetrics {
    fn default() -> Self {
        Self::new()
    }
}

impl InternalMetrics {
    /// Create a new internal metrics registry
    pub fn new() -> Self {
        let metrics = Self {
            targets: Arc::new(RwLock::new(HashMap::new())),
            rules: Arc::new(RwLock::new(HashMap::new())),
            connections: Arc::new(ConnectionPoolMetrics::default()),
            config: Arc::new(ConfigMetrics::default()),
        };

        // Record initial config load timestamp
        metrics.config.last_reload_timestamp.set_to_current_time();

        metrics
    }

    /// Get or create metrics for a target
    pub fn target(&self, target: &str) -> TargetMetrics {
        {
            let targets = self.targets.read().expect("RwLock poisoned");
            if let Some(metrics) = targets.get(target) {
                return metrics.clone();
            }
        }

        let mut targets = self.targets.write().expect("RwLock poisoned");
        targets.entry(target.to_string()).or_default().clone()
    }

    /// Record a successful scrape for a target
    pub fn record_scrape_success(&self, target: &str, duration_seconds: f64) {
        let mut targets = self.targets.write().expect("RwLock poisoned");
        let metrics = targets.entry(target.to_string()).or_default();
        metrics.scrape_success_total.inc();
        metrics.scrape_duration_seconds.observe(duration_seconds);
    }

    /// Record a failed scrape for a target
    pub fn record_scrape_failure(&self, target: &str, duration_seconds: f64) {
        let mut targets = self.targets.write().expect("RwLock poisoned");
        let metrics = targets.entry(target.to_string()).or_default();
        metrics.scrape_failure_total.inc();
        metrics.scrape_duration_seconds.observe(duration_seconds);
    }

    /// Get or create metrics for a rule
    pub fn rule(&self, pattern: &str) -> RuleMetrics {
        {
            let rules = self.rules.read().expect("RwLock poisoned");
            if let Some(metrics) = rules.get(pattern) {
                return metrics.clone();
            }
        }

        let mut rules = self.rules.write().expect("RwLock poisoned");
        rules.entry(pattern.to_string()).or_default().clone()
    }

    /// Record a rule match
    pub fn record_rule_match(&self, pattern: &str) {
        let mut rules = self.rules.write().expect("RwLock poisoned");
        let metrics = rules.entry(pattern.to_string()).or_default();
        metrics.matches_total.inc();
    }

    /// Record a rule error
    pub fn record_rule_error(&self, pattern: &str) {
        let mut rules = self.rules.write().expect("RwLock poisoned");
        let metrics = rules.entry(pattern.to_string()).or_default();
        metrics.errors_total.inc();
    }

    /// Record a config reload
    pub fn record_config_reload(&self) {
        self.config.reload_total.inc();
        self.config.last_reload_timestamp.set_to_current_time();
    }

    /// Update connection pool metrics
    pub fn update_connections(&self, active: f64, idle: f64) {
        self.connections.active.set(active);
        self.connections.idle.set(idle);
    }

    /// Format all internal metrics as Prometheus metrics
    pub fn to_prometheus_metrics(&self) -> Vec<PrometheusMetric> {
        let mut metrics = Vec::new();

        // Per-target metrics
        {
            let targets = self.targets.read().expect("RwLock poisoned");
            for (target, target_metrics) in targets.iter() {
                // Scrape success counter
                metrics.push(
                    PrometheusMetric::new(
                        "rjmx_scrape_success_total",
                        target_metrics.scrape_success_total.get() as f64,
                    )
                    .with_type(MetricType::Counter)
                    .with_help("Total number of successful scrapes")
                    .with_label("target", target),
                );

                // Scrape failure counter
                metrics.push(
                    PrometheusMetric::new(
                        "rjmx_scrape_failure_total",
                        target_metrics.scrape_failure_total.get() as f64,
                    )
                    .with_type(MetricType::Counter)
                    .with_help("Total number of failed scrapes")
                    .with_label("target", target),
                );

                // Scrape duration histogram
                let histogram = &target_metrics.scrape_duration_seconds;
                for (bound, count) in histogram.get_buckets() {
                    let le = if bound.is_infinite() {
                        "+Inf".to_string()
                    } else {
                        format!("{}", bound)
                    };
                    metrics.push(
                        PrometheusMetric::new("rjmx_scrape_duration_seconds_bucket", count as f64)
                            .with_type(MetricType::Gauge)
                            .with_help("Histogram of scrape durations")
                            .with_label("target", target)
                            .with_label("le", &le),
                    );
                }
                metrics.push(
                    PrometheusMetric::new("rjmx_scrape_duration_seconds_sum", histogram.get_sum())
                        .with_type(MetricType::Gauge)
                        .with_help("Total sum of scrape durations")
                        .with_label("target", target),
                );
                metrics.push(
                    PrometheusMetric::new(
                        "rjmx_scrape_duration_seconds_count",
                        histogram.get_count() as f64,
                    )
                    .with_type(MetricType::Gauge)
                    .with_help("Total count of scrapes")
                    .with_label("target", target),
                );
            }
        }

        // Per-rule metrics
        {
            let rules = self.rules.read().expect("RwLock poisoned");
            for (pattern, rule_metrics) in rules.iter() {
                metrics.push(
                    PrometheusMetric::new(
                        "rjmx_rule_matches_total",
                        rule_metrics.matches_total.get() as f64,
                    )
                    .with_type(MetricType::Counter)
                    .with_help("Total number of rule matches")
                    .with_label("rule", pattern),
                );

                metrics.push(
                    PrometheusMetric::new(
                        "rjmx_rule_errors_total",
                        rule_metrics.errors_total.get() as f64,
                    )
                    .with_type(MetricType::Counter)
                    .with_help("Total number of rule errors")
                    .with_label("rule", pattern),
                );
            }
        }

        // Connection pool metrics
        metrics.push(
            PrometheusMetric::new(
                "rjmx_http_connections_active",
                self.connections.active.get(),
            )
            .with_type(MetricType::Gauge)
            .with_help("Number of active HTTP connections"),
        );

        metrics.push(
            PrometheusMetric::new("rjmx_http_connections_idle", self.connections.idle.get())
                .with_type(MetricType::Gauge)
                .with_help("Number of idle HTTP connections"),
        );

        // Config metrics
        metrics.push(
            PrometheusMetric::new(
                "rjmx_config_reload_total",
                self.config.reload_total.get() as f64,
            )
            .with_type(MetricType::Counter)
            .with_help("Total number of configuration reloads"),
        );

        metrics.push(
            PrometheusMetric::new(
                "rjmx_config_last_reload_timestamp",
                self.config.last_reload_timestamp.get(),
            )
            .with_type(MetricType::Gauge)
            .with_help("Unix timestamp of the last configuration reload"),
        );

        metrics
    }

    /// Format internal metrics as Prometheus exposition format string
    pub fn format_prometheus(&self) -> String {
        use crate::transformer::PrometheusFormatter;

        let metrics = self.to_prometheus_metrics();
        let formatter = PrometheusFormatter::new();
        formatter.format(&metrics)
    }
}

/// Global internal metrics instance
///
/// Use this for convenient access to internal metrics throughout the application.
static INTERNAL_METRICS: std::sync::OnceLock<InternalMetrics> = std::sync::OnceLock::new();

/// Get the global internal metrics instance
pub fn internal_metrics() -> &'static InternalMetrics {
    INTERNAL_METRICS.get_or_init(InternalMetrics::new)
}

/// Initialize or reset the global internal metrics
///
/// Note: This can only be called once. Subsequent calls will return the existing instance.
pub fn init_internal_metrics() -> &'static InternalMetrics {
    internal_metrics()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_counter_operations() {
        let counter = Counter::new();
        assert_eq!(counter.get(), 0);

        counter.inc();
        assert_eq!(counter.get(), 1);

        counter.inc_by(5);
        assert_eq!(counter.get(), 6);

        counter.reset();
        assert_eq!(counter.get(), 0);
    }

    #[test]
    fn test_gauge_operations() {
        let gauge = Gauge::new();
        assert_eq!(gauge.get(), 0.0);

        gauge.set(42.5);
        assert_eq!(gauge.get(), 42.5);

        gauge.inc(7.5);
        assert_eq!(gauge.get(), 50.0);

        gauge.dec(10.0);
        assert_eq!(gauge.get(), 40.0);
    }

    #[test]
    fn test_histogram_operations() {
        let histogram = Histogram::new(&[0.1, 0.5, 1.0]);

        histogram.observe(0.05);
        histogram.observe(0.3);
        histogram.observe(0.8);
        histogram.observe(2.0);

        assert_eq!(histogram.get_count(), 4);

        let buckets = histogram.get_buckets();
        // 0.05 <= 0.1
        assert_eq!(buckets[0], (0.1, 1));
        // 0.05, 0.3 <= 0.5
        assert_eq!(buckets[1], (0.5, 2));
        // 0.05, 0.3, 0.8 <= 1.0
        assert_eq!(buckets[2], (1.0, 3));
        // All values <= +Inf
        assert_eq!(buckets[3].1, 4);
    }

    #[test]
    fn test_histogram_default_buckets() {
        let histogram = Histogram::with_default_buckets();
        let buckets = histogram.get_buckets();

        // Should have all default buckets plus +Inf
        assert_eq!(buckets.len(), DEFAULT_HISTOGRAM_BUCKETS.len() + 1);
    }

    #[test]
    fn test_internal_metrics_target() {
        let metrics = InternalMetrics::new();

        metrics.record_scrape_success("target1", 0.05);
        metrics.record_scrape_success("target1", 0.10);
        metrics.record_scrape_failure("target1", 0.50);

        let target_metrics = metrics.target("target1");
        assert_eq!(target_metrics.scrape_success_total.get(), 2);
        assert_eq!(target_metrics.scrape_failure_total.get(), 1);
        assert_eq!(target_metrics.scrape_duration_seconds.get_count(), 3);
    }

    #[test]
    fn test_internal_metrics_rule() {
        let metrics = InternalMetrics::new();

        metrics.record_rule_match("pattern1");
        metrics.record_rule_match("pattern1");
        metrics.record_rule_error("pattern1");

        let rule_metrics = metrics.rule("pattern1");
        assert_eq!(rule_metrics.matches_total.get(), 2);
        assert_eq!(rule_metrics.errors_total.get(), 1);
    }

    #[test]
    fn test_internal_metrics_connections() {
        let metrics = InternalMetrics::new();

        metrics.update_connections(5.0, 10.0);

        assert_eq!(metrics.connections.active.get(), 5.0);
        assert_eq!(metrics.connections.idle.get(), 10.0);
    }

    #[test]
    fn test_internal_metrics_config() {
        let metrics = InternalMetrics::new();

        let initial_timestamp = metrics.config.last_reload_timestamp.get();
        assert!(initial_timestamp > 0.0);

        metrics.record_config_reload();

        assert_eq!(metrics.config.reload_total.get(), 1);
        assert!(metrics.config.last_reload_timestamp.get() >= initial_timestamp);
    }

    #[test]
    fn test_to_prometheus_metrics() {
        let metrics = InternalMetrics::new();

        metrics.record_scrape_success("test-target", 0.1);
        metrics.record_rule_match("test-pattern");
        metrics.update_connections(1.0, 2.0);

        let prometheus_metrics = metrics.to_prometheus_metrics();

        // Check that we have metrics for all categories
        let metric_names: Vec<&str> = prometheus_metrics.iter().map(|m| m.name.as_str()).collect();

        assert!(metric_names.contains(&"rjmx_scrape_success_total"));
        assert!(metric_names.contains(&"rjmx_scrape_failure_total"));
        assert!(metric_names.contains(&"rjmx_scrape_duration_seconds_bucket"));
        assert!(metric_names.contains(&"rjmx_scrape_duration_seconds_sum"));
        assert!(metric_names.contains(&"rjmx_scrape_duration_seconds_count"));
        assert!(metric_names.contains(&"rjmx_rule_matches_total"));
        assert!(metric_names.contains(&"rjmx_rule_errors_total"));
        assert!(metric_names.contains(&"rjmx_http_connections_active"));
        assert!(metric_names.contains(&"rjmx_http_connections_idle"));
        assert!(metric_names.contains(&"rjmx_config_reload_total"));
        assert!(metric_names.contains(&"rjmx_config_last_reload_timestamp"));
    }

    #[test]
    fn test_format_prometheus() {
        let metrics = InternalMetrics::new();
        metrics.record_scrape_success("localhost:8778", 0.05);

        let output = metrics.format_prometheus();

        assert!(output.contains("rjmx_scrape_success_total"));
        assert!(output.contains("target=\"localhost:8778\""));
        assert!(output.contains("# HELP"));
        assert!(output.contains("# TYPE"));
    }

    #[test]
    fn test_counter_clone() {
        let counter = Counter::new();
        counter.inc_by(42);

        let cloned = counter.clone();
        assert_eq!(cloned.get(), 42);

        // Ensure independence
        counter.inc();
        assert_eq!(counter.get(), 43);
        assert_eq!(cloned.get(), 42);
    }

    #[test]
    fn test_gauge_clone() {
        let gauge = Gauge::new();
        gauge.set(3.14);

        let cloned = gauge.clone();
        assert_eq!(cloned.get(), 3.14);

        // Ensure independence
        gauge.set(2.71);
        assert_eq!(gauge.get(), 2.71);
        assert_eq!(cloned.get(), 3.14);
    }

    #[test]
    fn test_histogram_clone() {
        let histogram = Histogram::new(&[1.0, 5.0, 10.0]);
        histogram.observe(2.0);
        histogram.observe(7.0);

        let cloned = histogram.clone();
        assert_eq!(cloned.get_count(), 2);

        // Ensure independence
        histogram.observe(1.0);
        assert_eq!(histogram.get_count(), 3);
        assert_eq!(cloned.get_count(), 2);
    }
}
