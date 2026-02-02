//! Prometheus Exposition Format output
//!
//! This module handles formatting of Prometheus metrics into the text
//! exposition format (version 0.0.4).
//!
//! # Format Specification
//!
//! ```text
//! # HELP <metric_name> <help_text>
//! # TYPE <metric_name> <type>
//! <metric_name>{<label1>="<value1>",<label2>="<value2>"} <value> [<timestamp>]
//! ```

use std::collections::{HashMap, HashSet};

use super::engine::PrometheusMetric;

/// Prometheus exposition format formatter
///
/// Formats `PrometheusMetric` instances into the Prometheus text format.
///
/// # Example
///
/// ```ignore
/// use rjmx_exporter::transformer::{PrometheusFormatter, PrometheusMetric, MetricType};
///
/// let metrics = vec![
///     PrometheusMetric::new("jvm_memory_bytes", 123456789.0)
///         .with_type(MetricType::Gauge)
///         .with_help("JVM memory usage"),
/// ];
///
/// let formatter = PrometheusFormatter::new();
/// let output = formatter.format(&metrics);
/// ```
#[derive(Debug, Clone, Default)]
pub struct PrometheusFormatter {
    /// Include timestamp in output
    include_timestamp: bool,
}

impl PrometheusFormatter {
    /// Create a new formatter
    pub fn new() -> Self {
        Self::default()
    }

    /// Set whether to include timestamps in output
    pub fn with_timestamps(mut self, include: bool) -> Self {
        self.include_timestamp = include;
        self
    }

    /// Format metrics into Prometheus exposition format
    ///
    /// # Arguments
    ///
    /// * `metrics` - Slice of metrics to format
    ///
    /// # Returns
    ///
    /// A string containing all metrics in Prometheus text format.
    ///
    /// # Notes
    ///
    /// - HELP and TYPE lines are emitted once per unique metric name
    /// - Labels are sorted alphabetically for deterministic output
    /// - Metrics with the same name are grouped together
    pub fn format(&self, metrics: &[PrometheusMetric]) -> String {
        if metrics.is_empty() {
            return String::new();
        }

        let mut output = String::with_capacity(metrics.len() * 100);
        let mut seen_metrics: HashSet<String> = HashSet::new();

        // Group metrics by name for proper HELP/TYPE ordering
        let grouped = Self::group_by_name(metrics);

        for (name, group) in grouped {
            // HELP/TYPE are emitted once per metric name
            if !seen_metrics.contains(&name) {
                seen_metrics.insert(name.clone());

                // HELP line
                if let Some(help) = &group[0].help {
                    output.push_str(&format!("# HELP {} {}\n", name, Self::escape_help(help)));
                }

                // TYPE line
                output.push_str(&format!(
                    "# TYPE {} {}\n",
                    name,
                    group[0].metric_type.as_str()
                ));
            }

            // Metric lines
            for metric in group {
                output.push_str(&self.format_metric_line(metric));
                output.push('\n');
            }
        }

        output
    }

    /// Group metrics by name, preserving order of first occurrence
    fn group_by_name(metrics: &[PrometheusMetric]) -> Vec<(String, Vec<&PrometheusMetric>)> {
        let mut groups: HashMap<String, Vec<&PrometheusMetric>> = HashMap::new();
        let mut order: Vec<String> = Vec::new();

        for metric in metrics {
            if !groups.contains_key(&metric.name) {
                order.push(metric.name.clone());
            }
            groups.entry(metric.name.clone()).or_default().push(metric);
        }

        order
            .into_iter()
            .filter_map(|name| groups.remove(&name).map(|g| (name, g)))
            .collect()
    }

    /// Format a single metric line
    fn format_metric_line(&self, metric: &PrometheusMetric) -> String {
        let mut line = metric.name.clone();

        // Labels (sorted for deterministic output)
        if !metric.labels.is_empty() {
            let mut sorted_labels: Vec<(&String, &String)> = metric.labels.iter().collect();
            sorted_labels.sort_by_key(|(k, _)| *k);

            let label_pairs: Vec<String> = sorted_labels
                .iter()
                .map(|(k, v)| format!("{}=\"{}\"", k, Self::escape_label_value(v)))
                .collect();

            line.push('{');
            line.push_str(&label_pairs.join(","));
            line.push('}');
        }

        // Value
        line.push(' ');
        line.push_str(&Self::format_value(metric.value));

        // Timestamp (optional)
        if self.include_timestamp {
            if let Some(ts) = metric.timestamp {
                line.push(' ');
                line.push_str(&ts.to_string());
            }
        }

        line
    }

    /// Format a numeric value for Prometheus
    ///
    /// - NaN → "NaN"
    /// - +Inf → "+Inf"
    /// - -Inf → "-Inf"
    /// - Integers are formatted without decimal point
    /// - Large/small floats use scientific notation
    fn format_value(value: f64) -> String {
        if value.is_nan() {
            "NaN".to_string()
        } else if value.is_infinite() {
            if value.is_sign_positive() {
                "+Inf".to_string()
            } else {
                "-Inf".to_string()
            }
        } else if value.fract() == 0.0 && value.abs() < 1e15 {
            // Format as integer if no fractional part and not too large
            format!("{}", value as i64)
        } else if value.abs() >= 1e6 || (value.abs() < 1e-3 && value != 0.0) {
            // Use scientific notation for very large or very small numbers
            format!("{:e}", value)
        } else {
            // Standard decimal format
            format!("{}", value)
        }
    }

    /// Escape help text
    ///
    /// Escapes backslash and newline characters.
    fn escape_help(help: &str) -> String {
        help.replace('\\', "\\\\").replace('\n', "\\n")
    }

    /// Escape label value
    ///
    /// Escapes backslash, double-quote, and newline characters.
    fn escape_label_value(value: &str) -> String {
        let mut escaped = String::with_capacity(value.len());
        for c in value.chars() {
            match c {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                '\n' => escaped.push_str("\\n"),
                _ => escaped.push(c),
            }
        }
        escaped
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transformer::rules::MetricType;

    #[test]
    fn test_format_simple_metric() {
        let metrics = vec![PrometheusMetric::new("test_metric", 42.0)
            .with_type(MetricType::Gauge)
            .with_help("A test metric")];

        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        assert!(output.contains("# HELP test_metric A test metric"));
        assert!(output.contains("# TYPE test_metric gauge"));
        assert!(output.contains("test_metric 42"));
    }

    #[test]
    fn test_format_metric_with_labels() {
        let metrics = vec![PrometheusMetric::new("test_metric", 100.0)
            .with_type(MetricType::Counter)
            .with_label("env", "prod")
            .with_label("host", "server1")];

        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        assert!(output.contains("# TYPE test_metric counter"));
        // Labels should be sorted alphabetically
        assert!(output.contains("test_metric{env=\"prod\",host=\"server1\"} 100"));
    }

    #[test]
    fn test_format_multiple_metrics_same_name() {
        let metrics = vec![
            PrometheusMetric::new("http_requests_total", 1000.0)
                .with_type(MetricType::Counter)
                .with_help("Total HTTP requests")
                .with_label("method", "GET"),
            PrometheusMetric::new("http_requests_total", 500.0)
                .with_type(MetricType::Counter)
                .with_label("method", "POST"),
        ];

        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        // HELP and TYPE should appear only once
        assert_eq!(output.matches("# HELP http_requests_total").count(), 1);
        assert_eq!(output.matches("# TYPE http_requests_total").count(), 1);

        // Both metric lines should be present
        assert!(output.contains("http_requests_total{method=\"GET\"} 1000"));
        assert!(output.contains("http_requests_total{method=\"POST\"} 500"));
    }

    #[test]
    fn test_format_with_timestamp() {
        let metrics = vec![PrometheusMetric::new("test_metric", 42.0)
            .with_type(MetricType::Gauge)
            .with_timestamp(1609459200000)];

        let formatter = PrometheusFormatter::new().with_timestamps(true);
        let output = formatter.format(&metrics);

        assert!(output.contains("test_metric 42 1609459200000"));
    }

    #[test]
    fn test_format_without_timestamp() {
        let metrics = vec![PrometheusMetric::new("test_metric", 42.0)
            .with_type(MetricType::Gauge)
            .with_timestamp(1609459200000)];

        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        // Should not contain timestamp
        assert!(!output.contains("1609459200000"));
        assert!(output.contains("test_metric 42\n"));
    }

    #[test]
    fn test_format_value_nan() {
        assert_eq!(PrometheusFormatter::format_value(f64::NAN), "NaN");
    }

    #[test]
    fn test_format_value_infinity() {
        assert_eq!(PrometheusFormatter::format_value(f64::INFINITY), "+Inf");
        assert_eq!(PrometheusFormatter::format_value(f64::NEG_INFINITY), "-Inf");
    }

    #[test]
    fn test_format_value_integer() {
        assert_eq!(PrometheusFormatter::format_value(42.0), "42");
        assert_eq!(PrometheusFormatter::format_value(0.0), "0");
        assert_eq!(PrometheusFormatter::format_value(-100.0), "-100");
    }

    #[test]
    fn test_format_value_decimal() {
        let formatted = PrometheusFormatter::format_value(1.23456);
        assert!(formatted.starts_with("1.23"));
    }

    #[test]
    fn test_format_value_scientific() {
        // Very large number
        let formatted = PrometheusFormatter::format_value(1.23e10);
        assert!(formatted.contains('e') || formatted.contains("12300000000"));

        // Very small number
        let formatted = PrometheusFormatter::format_value(1.23e-6);
        assert!(formatted.contains('e') || formatted.contains("0.00000123"));
    }

    #[test]
    fn test_escape_help() {
        assert_eq!(PrometheusFormatter::escape_help("simple"), "simple");
        assert_eq!(
            PrometheusFormatter::escape_help("line1\nline2"),
            "line1\\nline2"
        );
        assert_eq!(
            PrometheusFormatter::escape_help("path\\to\\file"),
            "path\\\\to\\\\file"
        );
    }

    #[test]
    fn test_escape_label_value() {
        assert_eq!(PrometheusFormatter::escape_label_value("simple"), "simple");
        assert_eq!(
            PrometheusFormatter::escape_label_value("with\"quote"),
            "with\\\"quote"
        );
        assert_eq!(
            PrometheusFormatter::escape_label_value("with\\backslash"),
            "with\\\\backslash"
        );
        assert_eq!(
            PrometheusFormatter::escape_label_value("with\nnewline"),
            "with\\nnewline"
        );
        assert_eq!(
            PrometheusFormatter::escape_label_value("all\"\\\n"),
            "all\\\"\\\\\\n"
        );
    }

    #[test]
    fn test_format_empty_metrics() {
        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&[]);
        assert!(output.is_empty());
    }

    #[test]
    fn test_format_metric_without_help() {
        let metrics = vec![PrometheusMetric::new("test_metric", 42.0).with_type(MetricType::Gauge)];

        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        // Should not have HELP line
        assert!(!output.contains("# HELP"));
        // Should have TYPE line
        assert!(output.contains("# TYPE test_metric gauge"));
    }

    #[test]
    fn test_format_metric_no_labels() {
        let metrics = vec![PrometheusMetric::new("test_metric", 42.0)];

        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        // Should not have curly braces
        assert!(!output.contains('{'));
        assert!(!output.contains('}'));
    }

    #[test]
    fn test_jmx_exporter_compatible_output() {
        let metrics = vec![
            PrometheusMetric::new("jvm_memory_heap_used_bytes", 123456789.0)
                .with_type(MetricType::Gauge)
                .with_help("JVM heap memory used")
                .with_label("area", "heap"),
            PrometheusMetric::new("jvm_memory_heap_max_bytes", 536870912.0)
                .with_type(MetricType::Gauge)
                .with_help("JVM heap memory max")
                .with_label("area", "heap"),
        ];

        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        // Verify output matches expected jmx_exporter format
        assert!(output.contains("# HELP jvm_memory_heap_used_bytes JVM heap memory used"));
        assert!(output.contains("# TYPE jvm_memory_heap_used_bytes gauge"));
        assert!(output.contains("jvm_memory_heap_used_bytes{area=\"heap\"} 123456789"));
    }

    #[test]
    fn test_format_preserves_metric_order() {
        let metrics = vec![
            PrometheusMetric::new("zebra_metric", 1.0),
            PrometheusMetric::new("alpha_metric", 2.0),
            PrometheusMetric::new("middle_metric", 3.0),
        ];

        let formatter = PrometheusFormatter::new();
        let output = formatter.format(&metrics);

        // Find positions of each metric
        let zebra_pos = output.find("zebra_metric").unwrap();
        let alpha_pos = output.find("alpha_metric").unwrap();
        let middle_pos = output.find("middle_metric").unwrap();

        // Order should be preserved (zebra first, then alpha, then middle)
        assert!(zebra_pos < alpha_pos);
        assert!(alpha_pos < middle_pos);
    }
}
