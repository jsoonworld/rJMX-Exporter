//! Transform Engine - MBean to Prometheus metric conversion
//!
//! This module provides the core transformation logic that converts
//! JMX MBean data into Prometheus exposition format.

use std::collections::HashMap;

use crate::collector::{AttributeValue, JolokiaResponse, MBeanValue, ObjectName};
use crate::error::TransformError;

use super::rules::{MetricType, RuleSet};

/// Transform Engine configuration and state
///
/// The engine holds compiled rules and provides methods to transform
/// JMX MBean data into Prometheus metrics.
#[derive(Debug, Clone)]
pub struct TransformEngine {
    /// Compiled rule set
    rules: RuleSet,
    /// Convert metric names to lowercase
    lowercase_names: bool,
    /// Convert label names to lowercase
    lowercase_labels: bool,
}

impl TransformEngine {
    /// Create a new TransformEngine with the given rules
    ///
    /// # Arguments
    ///
    /// * `rules` - The rule set to use for transformation
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rjmx_exporter::transformer::{TransformEngine, RuleSet};
    ///
    /// let rules = RuleSet::new();
    /// let engine = TransformEngine::new(rules);
    /// ```
    pub fn new(rules: RuleSet) -> Self {
        Self {
            rules,
            lowercase_names: false,
            lowercase_labels: false,
        }
    }

    /// Create an empty transform engine
    pub fn empty() -> Self {
        Self::new(RuleSet::new())
    }

    /// Set whether to lowercase metric names
    pub fn with_lowercase_names(mut self, lowercase: bool) -> Self {
        self.lowercase_names = lowercase;
        self
    }

    /// Set whether to lowercase label names
    pub fn with_lowercase_labels(mut self, lowercase: bool) -> Self {
        self.lowercase_labels = lowercase;
        self
    }

    /// Get a reference to the rule set
    pub fn rules(&self) -> &RuleSet {
        &self.rules
    }

    /// Transform Jolokia responses into Prometheus metrics
    ///
    /// # Arguments
    ///
    /// * `responses` - Slice of Jolokia responses to transform
    ///
    /// # Returns
    ///
    /// A vector of Prometheus metrics ready for formatting
    pub fn transform(
        &self,
        responses: &[JolokiaResponse],
    ) -> Result<Vec<PrometheusMetric>, TransformError> {
        let mut metrics = Vec::new();

        for response in responses {
            // Skip error responses
            if response.status != 200 {
                tracing::debug!(
                    mbean = %response.request.mbean,
                    status = response.status,
                    error = ?response.error,
                    "Skipping error response"
                );
                continue;
            }

            let response_metrics = self.transform_response(response)?;
            metrics.extend(response_metrics);
        }

        Ok(metrics)
    }

    /// Transform a single Jolokia response
    fn transform_response(
        &self,
        response: &JolokiaResponse,
    ) -> Result<Vec<PrometheusMetric>, TransformError> {
        // Extract attribute(s) from RequestInfo
        // Jolokia supports both single attribute (string) and multiple attributes (array)
        let attributes = self.extract_attributes(&response.request.attribute);

        match &response.value {
            MBeanValue::Number(n) => {
                // For single numeric value, use the first attribute if available
                let attr = attributes.first().map(|s| s.as_str());
                self.transform_simple(&response.request.mbean, attr, *n)
            }
            MBeanValue::Composite(map) => {
                // For composite values, handle both single and multiple attributes
                if attributes.is_empty() {
                    self.transform_composite(&response.request.mbean, None, map)
                } else if attributes.len() == 1 {
                    self.transform_composite(
                        &response.request.mbean,
                        Some(attributes[0].as_str()),
                        map,
                    )
                } else {
                    // Multiple attributes: the composite map keys are the attribute names
                    // Each attribute maps to its value in the composite
                    let mut metrics = Vec::new();
                    for attr in &attributes {
                        if let Some(attr_value) = map.get(attr) {
                            match attr_value {
                                AttributeValue::Integer(n) => {
                                    let mut m = self.transform_simple(
                                        &response.request.mbean,
                                        Some(attr.as_str()),
                                        *n as f64,
                                    )?;
                                    metrics.append(&mut m);
                                }
                                AttributeValue::Float(n) => {
                                    let mut m = self.transform_simple(
                                        &response.request.mbean,
                                        Some(attr.as_str()),
                                        *n,
                                    )?;
                                    metrics.append(&mut m);
                                }
                                AttributeValue::Object(nested) => {
                                    let mut m = self.transform_composite(
                                        &response.request.mbean,
                                        Some(attr.as_str()),
                                        nested,
                                    )?;
                                    metrics.append(&mut m);
                                }
                                _ => {}
                            }
                        }
                    }
                    Ok(metrics)
                }
            }
            MBeanValue::Wildcard(wildcard) => self.transform_wildcard(wildcard),
            _ => Ok(vec![]),
        }
    }

    /// Extract attributes from RequestInfo.attribute field
    /// Handles both string (single attribute) and array (multiple attributes)
    fn extract_attributes(&self, attribute: &Option<serde_json::Value>) -> Vec<String> {
        match attribute {
            Some(serde_json::Value::String(s)) => vec![s.clone()],
            Some(serde_json::Value::Array(arr)) => arr
                .iter()
                .filter_map(|v| v.as_str().map(|s| s.to_string()))
                .collect(),
            _ => vec![],
        }
    }

    /// Transform a simple numeric value
    fn transform_simple(
        &self,
        mbean: &str,
        attribute: Option<&str>,
        value: f64,
    ) -> Result<Vec<PrometheusMetric>, TransformError> {
        let flattened = self.flatten_mbean_name(mbean, attribute);

        if let Some(rule_match) = self.rules.find_match(&flattened).map_err(|e| {
            // Convert rules::RuleError to crate::error::RuleError, preserving original context
            match e {
                super::rules::RuleError::InvalidPattern { pattern, source } => {
                    TransformError::Rule(crate::error::RuleError::InvalidPattern {
                        pattern,
                        source,
                    })
                }
                super::rules::RuleError::UnsupportedJavaFeature { pattern, feature } => {
                    TransformError::Rule(crate::error::RuleError::UnsupportedSyntax {
                        pattern,
                        feature,
                    })
                }
                super::rules::RuleError::CompilationFailed(msg) => {
                    TransformError::Rule(crate::error::RuleError::InvalidPattern {
                        pattern: msg.clone(),
                        source: regex::Error::Syntax(msg),
                    })
                }
                super::rules::RuleError::InvalidNameTemplate { template, reason } => {
                    TransformError::InvalidMetricName {
                        name: template,
                        reason,
                    }
                }
                super::rules::RuleError::ValidationError(msg) => {
                    TransformError::InvalidMetricName {
                        name: String::new(),
                        reason: msg,
                    }
                }
            }
        })? {
            // Warn if the rule has a 'value' field set (not yet implemented)
            if rule_match.value().is_some() {
                tracing::warn!(
                    rule_pattern = %rule_match.rule.pattern,
                    "Rule 'value' field is not yet implemented, using raw attribute value"
                );
            }

            let mut metric_name = rule_match.metric_name();
            if self.lowercase_names {
                metric_name = metric_name.to_lowercase();
            }

            let validated_name = self.validate_metric_name(&metric_name)?;

            let mut labels = rule_match.labels();
            if self.lowercase_labels {
                labels = labels
                    .into_iter()
                    .map(|(k, v)| (k.to_lowercase(), v))
                    .collect();
            }
            let validated_labels = self.validate_labels(&labels)?;

            let final_value = match rule_match.value_factor() {
                Some(factor) => value * factor,
                None => value,
            };

            Ok(vec![PrometheusMetric {
                name: validated_name,
                metric_type: rule_match.metric_type(),
                help: rule_match.help().map(|s| s.to_string()),
                labels: validated_labels,
                value: final_value,
                timestamp: None,
            }])
        } else {
            // No matching rule - skip this metric
            tracing::trace!(mbean = %mbean, "No matching rule found");
            Ok(vec![])
        }
    }

    /// Transform a composite value (e.g., HeapMemoryUsage)
    ///
    /// For composite values, the flattened name format is:
    /// `domain<key=value><attribute><composite_key>`
    ///
    /// Example: For MBean "java.lang:type=Memory" with attribute "HeapMemoryUsage"
    /// and composite key "used", the flattened name will be:
    /// `java.lang<type=Memory><HeapMemoryUsage><used>`
    fn transform_composite(
        &self,
        mbean: &str,
        attribute: Option<&str>,
        composite: &HashMap<String, AttributeValue>,
    ) -> Result<Vec<PrometheusMetric>, TransformError> {
        let mut metrics = Vec::new();

        for (key, value) in composite {
            if let Some(num) = value.as_f64() {
                // Build the full attribute path: attribute + composite key
                // e.g., "HeapMemoryUsage" + "used" -> flatten as <HeapMemoryUsage><used>
                let full_attr = match attribute {
                    Some(attr) => format!("{}<{}>", attr, key),
                    None => key.clone(),
                };
                let mut new_metrics = self.transform_simple(mbean, Some(&full_attr), num)?;
                metrics.append(&mut new_metrics);
            }
        }

        Ok(metrics)
    }

    /// Transform a wildcard response
    ///
    /// For wildcard responses, we need to handle each attribute type appropriately:
    /// - Numeric values (Integer/Float) -> transform_simple
    /// - Object values (nested composites) -> transform_composite recursively
    fn transform_wildcard(
        &self,
        wildcard: &HashMap<String, HashMap<String, AttributeValue>>,
    ) -> Result<Vec<PrometheusMetric>, TransformError> {
        let mut metrics = Vec::new();

        for (mbean_name, attrs) in wildcard {
            // Handle each attribute based on its type
            for (attr_name, attr_value) in attrs {
                match attr_value {
                    AttributeValue::Integer(n) => {
                        let mut m = self.transform_simple(mbean_name, Some(attr_name), *n as f64)?;
                        metrics.append(&mut m);
                    }
                    AttributeValue::Float(n) => {
                        let mut m = self.transform_simple(mbean_name, Some(attr_name), *n)?;
                        metrics.append(&mut m);
                    }
                    AttributeValue::Object(nested) => {
                        // Recursively handle nested composite objects
                        let mut m = self.transform_composite(mbean_name, Some(attr_name), nested)?;
                        metrics.append(&mut m);
                    }
                    _ => {
                        // Skip non-numeric types (String, Boolean, Array, Null)
                    }
                }
            }
        }

        Ok(metrics)
    }

    /// Flatten MBean name to jmx_exporter format
    ///
    /// Format: `domain<key1=value1><key2=value2><attribute>`
    ///
    /// Example: "java.lang:type=Memory" with attribute "HeapMemoryUsage<used>"
    /// becomes: "java.lang<type=Memory><HeapMemoryUsage><used>"
    fn flatten_mbean_name(&self, mbean: &str, attribute: Option<&str>) -> String {
        // Parse ObjectName to get domain and properties
        let object_name = match ObjectName::parse(mbean) {
            Ok(on) => on,
            Err(_) => {
                // Fallback: just use the raw name
                if let Some(attr) = attribute {
                    return format!("{}<{}>", mbean, attr);
                }
                return mbean.to_string();
            }
        };

        let mut result = object_name.domain.clone();

        // Sort properties for deterministic output
        let mut props: Vec<_> = object_name.properties.iter().collect();
        props.sort_by_key(|(k, _)| *k);

        // Add properties in <key=value> format
        for (key, value) in props {
            result.push_str(&format!("<{}={}>", key, value));
        }

        // Add attribute if present
        // The attribute may already contain nested <> for composite keys
        // e.g., "HeapMemoryUsage<used>" should become "<HeapMemoryUsage><used>"
        if let Some(attr) = attribute {
            // Check if attribute already has angle brackets (composite path)
            if let Some(bracket_pos) = attr.find('<') {
                // Split at the first '<' to get base attribute and composite key
                let base_attr = &attr[..bracket_pos];
                let composite_part = &attr[bracket_pos..];
                result.push_str(&format!("<{}>{}", base_attr, composite_part));
            } else {
                result.push_str(&format!("<{}>", attr));
            }
        }

        result
    }

    /// Validate and sanitize Prometheus metric name
    ///
    /// Prometheus metric names must match: `[a-zA-Z_:][a-zA-Z0-9_:]*`
    fn validate_metric_name(&self, name: &str) -> Result<String, TransformError> {
        use std::sync::OnceLock;

        static METRIC_NAME_RE: OnceLock<regex::Regex> = OnceLock::new();
        let re = METRIC_NAME_RE.get_or_init(|| {
            regex::Regex::new(r"^[a-zA-Z_:][a-zA-Z0-9_:]*$").expect("invalid metric name regex")
        });

        if re.is_match(name) {
            return Ok(name.to_string());
        }

        // Sanitize: replace invalid chars with underscore
        let sanitized: String = name
            .chars()
            .enumerate()
            .map(|(i, c)| {
                if i == 0 {
                    if c.is_ascii_alphabetic() || c == '_' || c == ':' {
                        c
                    } else {
                        '_'
                    }
                } else if c.is_ascii_alphanumeric() || c == '_' || c == ':' {
                    c
                } else {
                    '_'
                }
            })
            .collect();

        // Ensure the name doesn't start with a digit after sanitization
        let final_name = if sanitized.starts_with(|c: char| c.is_ascii_digit()) {
            format!("_{}", sanitized)
        } else {
            sanitized
        };

        tracing::warn!(
            original = %name,
            sanitized = %final_name,
            "Metric name sanitized to match Prometheus naming rules"
        );

        Ok(final_name)
    }

    /// Validate and sanitize label names
    ///
    /// Prometheus label names must match: `[a-zA-Z_][a-zA-Z0-9_]*`
    fn validate_labels(
        &self,
        labels: &HashMap<String, String>,
    ) -> Result<HashMap<String, String>, TransformError> {
        use std::sync::OnceLock;

        static LABEL_NAME_RE: OnceLock<regex::Regex> = OnceLock::new();
        let re = LABEL_NAME_RE.get_or_init(|| {
            regex::Regex::new(r"^[a-zA-Z_][a-zA-Z0-9_]*$").expect("invalid label name regex")
        });

        let mut validated = HashMap::new();
        for (k, v) in labels {
            let key = if re.is_match(k) {
                k.clone()
            } else {
                let sanitized: String = k
                    .chars()
                    .enumerate()
                    .map(|(i, c)| {
                        if i == 0 {
                            if c.is_ascii_alphabetic() || c == '_' {
                                c
                            } else {
                                '_'
                            }
                        } else if c.is_ascii_alphanumeric() || c == '_' {
                            c
                        } else {
                            '_'
                        }
                    })
                    .collect();

                tracing::warn!(
                    original = %k,
                    sanitized = %sanitized,
                    "Label name sanitized"
                );
                sanitized
            };
            validated.insert(key, v.clone());
        }

        Ok(validated)
    }
}

impl Default for TransformEngine {
    fn default() -> Self {
        Self::empty()
    }
}

/// A single Prometheus metric ready for output
#[derive(Debug, Clone)]
pub struct PrometheusMetric {
    /// Metric name
    pub name: String,
    /// Metric type (gauge, counter, untyped)
    pub metric_type: MetricType,
    /// Help text
    pub help: Option<String>,
    /// Labels
    pub labels: HashMap<String, String>,
    /// Metric value
    pub value: f64,
    /// Optional timestamp (milliseconds since epoch)
    pub timestamp: Option<i64>,
}

impl PrometheusMetric {
    /// Create a new metric
    pub fn new(name: impl Into<String>, value: f64) -> Self {
        Self {
            name: name.into(),
            metric_type: MetricType::Untyped,
            help: None,
            labels: HashMap::new(),
            value,
            timestamp: None,
        }
    }

    /// Set the metric type
    pub fn with_type(mut self, metric_type: MetricType) -> Self {
        self.metric_type = metric_type;
        self
    }

    /// Set help text
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Add a label
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Set timestamp
    pub fn with_timestamp(mut self, timestamp: i64) -> Self {
        self.timestamp = Some(timestamp);
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::transformer::rules::{Rule, RuleSet};

    fn create_test_engine() -> TransformEngine {
        let mut ruleset = RuleSet::new();
        ruleset.add(
            Rule::builder(r"java\.lang<type=Memory><HeapMemoryUsage><(\w+)>")
                .name("jvm_memory_heap_$1_bytes")
                .metric_type(MetricType::Gauge)
                .help("JVM heap memory $1")
                .label("area", "heap")
                .build(),
        );
        ruleset.add(
            Rule::builder(r"java\.lang<type=Threading><(\w+)>")
                .name("jvm_threads_$1")
                .metric_type(MetricType::Gauge)
                .build(),
        );
        TransformEngine::new(ruleset)
    }

    #[test]
    fn test_transform_simple() {
        let engine = create_test_engine();

        // Test transform_simple directly with the attribute passed correctly
        // This tests the core transformation logic independent of response parsing
        let metrics = engine
            .transform_simple("java.lang:type=Threading", Some("ThreadCount"), 42.0)
            .unwrap();

        // Verify the transformation produces the expected metric
        assert_eq!(metrics.len(), 1, "Expected exactly one metric");
        assert_eq!(metrics[0].name, "jvm_threads_ThreadCount");
        assert_eq!(metrics[0].value, 42.0);
        assert_eq!(metrics[0].metric_type, MetricType::Gauge);
    }

    #[test]
    fn test_flatten_mbean_name() {
        let engine = TransformEngine::empty();

        let flattened = engine.flatten_mbean_name("java.lang:type=Memory", Some("HeapMemoryUsage"));
        assert!(flattened.contains("java.lang"));
        assert!(flattened.contains("type=Memory"));
        assert!(flattened.contains("HeapMemoryUsage"));
    }

    #[test]
    fn test_validate_metric_name() {
        let engine = TransformEngine::empty();

        // Valid names
        assert_eq!(
            engine.validate_metric_name("valid_name").unwrap(),
            "valid_name"
        );
        assert_eq!(
            engine.validate_metric_name("valid:name").unwrap(),
            "valid:name"
        );

        // Invalid names get sanitized
        let result = engine.validate_metric_name("invalid-name").unwrap();
        assert!(!result.contains('-'));

        let result = engine.validate_metric_name("123invalid").unwrap();
        assert!(result.starts_with('_'));
    }

    #[test]
    fn test_validate_labels() {
        let engine = TransformEngine::empty();

        let mut labels = HashMap::new();
        labels.insert("valid_label".to_string(), "value".to_string());
        labels.insert("invalid-label".to_string(), "value2".to_string());

        let validated = engine.validate_labels(&labels).unwrap();
        assert!(validated.contains_key("valid_label"));
        // invalid-label should be sanitized
        assert!(validated.keys().any(|k| k.contains("invalid")));
    }

    #[test]
    fn test_transform_composite_with_attribute() {
        let engine = create_test_engine();

        let mut composite = HashMap::new();
        composite.insert("used".to_string(), AttributeValue::Integer(123456789));
        composite.insert("max".to_string(), AttributeValue::Integer(536870912));
        composite.insert(
            "name".to_string(),
            AttributeValue::String("test".to_string()),
        );

        // Now pass the attribute "HeapMemoryUsage" to match the rule pattern
        let metrics = engine
            .transform_composite("java.lang:type=Memory", Some("HeapMemoryUsage"), &composite)
            .unwrap();

        // Should produce 2 metrics (used and max), string "name" is skipped
        assert_eq!(metrics.len(), 2, "Expected two numeric metrics");

        // Verify the metrics have correct names
        let metric_names: Vec<&str> = metrics.iter().map(|m| m.name.as_str()).collect();
        assert!(
            metric_names.contains(&"jvm_memory_heap_used_bytes"),
            "Expected jvm_memory_heap_used_bytes, got {:?}",
            metric_names
        );
        assert!(
            metric_names.contains(&"jvm_memory_heap_max_bytes"),
            "Expected jvm_memory_heap_max_bytes, got {:?}",
            metric_names
        );
    }

    #[test]
    fn test_lowercase_options() {
        let engine = TransformEngine::empty()
            .with_lowercase_names(true)
            .with_lowercase_labels(true);

        assert!(engine.lowercase_names);
        assert!(engine.lowercase_labels);
    }

    #[test]
    fn test_prometheus_metric_builder() {
        let metric = PrometheusMetric::new("test_metric", 42.0)
            .with_type(MetricType::Gauge)
            .with_help("Test help")
            .with_label("env", "prod")
            .with_timestamp(1609459200000);

        assert_eq!(metric.name, "test_metric");
        assert_eq!(metric.metric_type, MetricType::Gauge);
        assert_eq!(metric.help, Some("Test help".to_string()));
        assert_eq!(metric.labels.get("env"), Some(&"prod".to_string()));
        assert_eq!(metric.timestamp, Some(1609459200000));
    }

    /// Test that verifies the fix for HIGH severity issue:
    /// RequestInfo.attribute is now correctly passed during transformation
    #[test]
    fn test_request_info_attribute_not_dropped() {
        use crate::collector::RequestInfo;

        let engine = create_test_engine();

        // Create a response with attribute in RequestInfo
        let responses = vec![JolokiaResponse {
            request: RequestInfo {
                mbean: "java.lang:type=Threading".to_string(),
                attribute: Some(serde_json::json!("ThreadCount")),
                request_type: "read".to_string(),
            },
            value: MBeanValue::Number(42.0),
            status: 200,
            timestamp: 1609459200,
            error: None,
            error_type: None,
        }];

        let metrics = engine.transform(&responses).unwrap();

        // The attribute from RequestInfo should be used in transformation
        assert_eq!(metrics.len(), 1, "Expected exactly one metric");
        assert_eq!(
            metrics[0].name, "jvm_threads_ThreadCount",
            "Attribute 'ThreadCount' should be included in flattened name"
        );
        assert_eq!(metrics[0].value, 42.0);
    }

    /// Test that verifies composite values include the attribute from RequestInfo
    #[test]
    fn test_composite_value_includes_request_attribute() {
        use crate::collector::RequestInfo;

        let engine = create_test_engine();

        // Create a composite response with HeapMemoryUsage attribute
        let mut composite_value = HashMap::new();
        composite_value.insert("used".to_string(), AttributeValue::Integer(52428800));
        composite_value.insert("max".to_string(), AttributeValue::Integer(536870912));

        let responses = vec![JolokiaResponse {
            request: RequestInfo {
                mbean: "java.lang:type=Memory".to_string(),
                attribute: Some(serde_json::json!("HeapMemoryUsage")),
                request_type: "read".to_string(),
            },
            value: MBeanValue::Composite(composite_value),
            status: 200,
            timestamp: 1609459200,
            error: None,
            error_type: None,
        }];

        let metrics = engine.transform(&responses).unwrap();

        // Should produce 2 metrics matching the pattern
        // java.lang<type=Memory><HeapMemoryUsage><used> -> jvm_memory_heap_used_bytes
        // java.lang<type=Memory><HeapMemoryUsage><max> -> jvm_memory_heap_max_bytes
        assert_eq!(metrics.len(), 2, "Expected two metrics for composite value");

        let metric_names: Vec<&str> = metrics.iter().map(|m| m.name.as_str()).collect();
        assert!(
            metric_names.contains(&"jvm_memory_heap_used_bytes"),
            "Expected jvm_memory_heap_used_bytes in {:?}",
            metric_names
        );
        assert!(
            metric_names.contains(&"jvm_memory_heap_max_bytes"),
            "Expected jvm_memory_heap_max_bytes in {:?}",
            metric_names
        );

        // Verify the labels are applied from the rule
        for metric in &metrics {
            assert_eq!(
                metric.labels.get("area"),
                Some(&"heap".to_string()),
                "Label 'area=heap' should be set from rule"
            );
        }
    }

    /// Test flatten_mbean_name with composite attribute path
    #[test]
    fn test_flatten_mbean_name_with_composite_path() {
        let engine = TransformEngine::empty();

        // When attribute already contains composite path (e.g., from transform_composite)
        let flattened =
            engine.flatten_mbean_name("java.lang:type=Memory", Some("HeapMemoryUsage<used>"));

        // Should produce: java.lang<type=Memory><HeapMemoryUsage><used>
        assert_eq!(
            flattened, "java.lang<type=Memory><HeapMemoryUsage><used>",
            "Composite attribute path should be properly formatted"
        );
    }

    /// Test that verifies array attribute handling for multiple attributes
    #[test]
    fn test_array_attribute_handling() {
        let engine = create_test_engine();

        // Test extract_attributes with string value
        let string_attr = Some(serde_json::json!("ThreadCount"));
        let attrs = engine.extract_attributes(&string_attr);
        assert_eq!(attrs, vec!["ThreadCount"]);

        // Test extract_attributes with array value
        let array_attr = Some(serde_json::json!(["ThreadCount", "PeakThreadCount"]));
        let attrs = engine.extract_attributes(&array_attr);
        assert_eq!(attrs, vec!["ThreadCount", "PeakThreadCount"]);

        // Test extract_attributes with None
        let none_attr: Option<serde_json::Value> = None;
        let attrs = engine.extract_attributes(&none_attr);
        assert!(attrs.is_empty());
    }

    /// Test transformation with multiple attributes in a single response
    #[test]
    fn test_transform_multiple_attributes() {
        use crate::collector::RequestInfo;

        let engine = create_test_engine();

        // Create a response with multiple attributes (array)
        let mut composite_value = HashMap::new();
        composite_value.insert("ThreadCount".to_string(), AttributeValue::Integer(42));
        composite_value.insert("PeakThreadCount".to_string(), AttributeValue::Integer(100));

        let responses = vec![JolokiaResponse {
            request: RequestInfo {
                mbean: "java.lang:type=Threading".to_string(),
                attribute: Some(serde_json::json!(["ThreadCount", "PeakThreadCount"])),
                request_type: "read".to_string(),
            },
            value: MBeanValue::Composite(composite_value),
            status: 200,
            timestamp: 1609459200,
            error: None,
            error_type: None,
        }];

        let metrics = engine.transform(&responses).unwrap();

        // Should produce metrics for both attributes
        let metric_names: Vec<&str> = metrics.iter().map(|m| m.name.as_str()).collect();
        assert!(
            metric_names.contains(&"jvm_threads_ThreadCount"),
            "Expected jvm_threads_ThreadCount in {:?}",
            metric_names
        );
        assert!(
            metric_names.contains(&"jvm_threads_PeakThreadCount"),
            "Expected jvm_threads_PeakThreadCount in {:?}",
            metric_names
        );
    }
}
