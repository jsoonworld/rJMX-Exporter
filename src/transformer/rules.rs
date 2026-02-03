//! Rule-based metric transformation module
//!
//! This module provides rule definitions for transforming JMX MBean data
//! into Prometheus metrics format. It supports pattern matching with regex,
//! metric type definitions, and label configuration.
//!
//! # Example
//!
//! ```ignore
//! use rjmx_exporter::transformer::rules::{Rule, RuleSet, MetricType};
//!
//! let rules = RuleSet::from_rules(vec![
//!     Rule::new(
//!         "java.lang<type=Memory><HeapMemoryUsage>(\\w+)",
//!         "jvm_memory_heap_$1_bytes",
//!         MetricType::Gauge,
//!     ),
//! ])?;
//!
//! rules.compile_all()?;
//! ```

use once_cell::sync::OnceCell;
use regex::Regex;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use std::collections::HashMap;
use thiserror::Error;

/// Errors that can occur during rule processing
#[derive(Error, Debug)]
pub enum RuleError {
    /// Invalid regex pattern
    #[error("Invalid regex pattern '{pattern}': {source}")]
    InvalidPattern {
        pattern: String,
        #[source]
        source: regex::Error,
    },

    /// Unsupported Java regex feature
    #[error("Unsupported Java regex feature in pattern '{pattern}': {feature}")]
    UnsupportedJavaFeature { pattern: String, feature: String },

    /// Pattern compilation failed
    #[error("Failed to compile pattern: {0}")]
    CompilationFailed(String),

    /// Invalid metric name template
    #[error("Invalid metric name template '{template}': {reason}")]
    InvalidNameTemplate { template: String, reason: String },

    /// Rule validation error
    #[error("Rule validation error: {0}")]
    ValidationError(String),
}

/// Result type for rule operations
pub type RuleResult<T> = Result<T, RuleError>;

/// Prometheus metric type
///
/// Defines the type of metric for Prometheus exposition format.
/// The default type is `Untyped` when not specified.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum MetricType {
    /// Gauge metric - a value that can go up and down
    Gauge,
    /// Counter metric - a monotonically increasing value
    Counter,
    /// Histogram metric - observations counted in buckets
    Histogram,
    /// Untyped metric - type is not specified
    #[default]
    Untyped,
}

impl MetricType {
    /// Returns the Prometheus type string representation
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rjmx_exporter::transformer::rules::MetricType;
    ///
    /// assert_eq!(MetricType::Gauge.as_str(), "gauge");
    /// assert_eq!(MetricType::Counter.as_str(), "counter");
    /// assert_eq!(MetricType::Histogram.as_str(), "histogram");
    /// assert_eq!(MetricType::Untyped.as_str(), "untyped");
    /// ```
    pub fn as_str(&self) -> &'static str {
        match self {
            MetricType::Gauge => "gauge",
            MetricType::Counter => "counter",
            MetricType::Histogram => "histogram",
            MetricType::Untyped => "untyped",
        }
    }
}

impl Serialize for MetricType {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: Serializer,
    {
        serializer.serialize_str(self.as_str())
    }
}

impl<'de> Deserialize<'de> for MetricType {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: Deserializer<'de>,
    {
        let s = String::deserialize(deserializer)?;
        match s.to_lowercase().as_str() {
            "gauge" => Ok(MetricType::Gauge),
            "counter" => Ok(MetricType::Counter),
            "histogram" => Ok(MetricType::Histogram),
            "untyped" => Ok(MetricType::Untyped),
            other => Err(serde::de::Error::custom(format!(
                "unknown metric type '{}', expected one of: gauge, counter, histogram, untyped",
                other
            ))),
        }
    }
}

impl std::fmt::Display for MetricType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

/// Metric transformation rule
///
/// A rule defines how to transform a JMX MBean attribute into a Prometheus metric.
/// It includes pattern matching, metric naming, type specification, and labels.
///
/// # Pattern Matching
///
/// The pattern uses regex to match MBean object names and attribute paths.
/// Capture groups can be referenced in the metric name using `$1`, `$2`, etc.
///
/// # Example Configuration (YAML)
///
/// ```yaml
/// pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
/// name: "jvm_memory_heap_$1_bytes"
/// type: gauge
/// help: "JVM heap memory usage"
/// labels:
///   area: "heap"
/// ```
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// Regex pattern for matching MBean object names and attributes
    ///
    /// Supports capture groups that can be referenced in the metric name.
    pub pattern: String,

    /// Output metric name template
    ///
    /// Supports `$1`, `$2`, etc. for capture group substitution.
    /// Also supports named groups via `$name` syntax.
    #[serde(default)]
    pub name: String,

    /// Prometheus metric type
    #[serde(rename = "type", default)]
    pub metric_type: MetricType,

    /// Static labels to add to the metric
    ///
    /// Keys and values can use capture group substitution.
    #[serde(default)]
    pub labels: HashMap<String, String>,

    /// Help text for the metric
    #[serde(default)]
    pub help: Option<String>,

    /// Value expression for extracting the metric value
    ///
    /// If not specified, the matched attribute value is used directly.
    #[serde(default)]
    pub value: Option<String>,

    /// Factor to multiply the value by
    ///
    /// Useful for unit conversions (e.g., milliseconds to seconds: 0.001)
    #[serde(rename = "valueFactor", default)]
    pub value_factor: Option<f64>,

    /// Compiled regex pattern (internal, not serialized)
    #[serde(skip)]
    compiled_pattern: OnceCell<Regex>,
}

impl Rule {
    /// Create a new rule with the given pattern, name, and metric type
    ///
    /// # Arguments
    ///
    /// * `pattern` - Regex pattern for matching MBeans
    /// * `name` - Output metric name template
    /// * `metric_type` - Prometheus metric type
    ///
    /// # Example
    ///
    /// ```ignore
    /// use rjmx_exporter::transformer::rules::{Rule, MetricType};
    ///
    /// let rule = Rule::new(
    ///     "java.lang<type=Memory><HeapMemoryUsage>(\\w+)",
    ///     "jvm_memory_heap_$1_bytes",
    ///     MetricType::Gauge,
    /// );
    /// ```
    pub fn new(
        pattern: impl Into<String>,
        name: impl Into<String>,
        metric_type: MetricType,
    ) -> Self {
        Self {
            pattern: pattern.into(),
            name: name.into(),
            metric_type,
            labels: HashMap::new(),
            help: None,
            value: None,
            value_factor: None,
            compiled_pattern: OnceCell::new(),
        }
    }

    /// Create a new rule builder for fluent configuration
    pub fn builder(pattern: impl Into<String>) -> RuleBuilder {
        RuleBuilder::new(pattern)
    }

    /// Add a label to the rule
    pub fn with_label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Set the help text
    pub fn with_help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Set the value expression
    pub fn with_value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Set the value factor
    pub fn with_value_factor(mut self, factor: f64) -> Self {
        self.value_factor = Some(factor);
        self
    }

    /// Compile the regex pattern
    ///
    /// This method lazily compiles the pattern on first call.
    /// Subsequent calls return the cached compiled regex.
    ///
    /// # Errors
    ///
    /// Returns `RuleError::InvalidPattern` if the pattern is not valid regex.
    pub fn compile(&self) -> RuleResult<&Regex> {
        self.compiled_pattern.get_or_try_init(|| {
            let converted = convert_java_regex(&self.pattern)?;
            Regex::new(&converted).map_err(|e| RuleError::InvalidPattern {
                pattern: self.pattern.clone(),
                source: e,
            })
        })
    }

    /// Get the compiled regex if already compiled, without attempting compilation
    pub fn get_compiled(&self) -> Option<&Regex> {
        self.compiled_pattern.get()
    }

    /// Check if the rule pattern has been compiled
    pub fn is_compiled(&self) -> bool {
        self.compiled_pattern.get().is_some()
    }

    /// Check if the rule matches the given input string
    ///
    /// # Arguments
    ///
    /// * `input` - The MBean object name or attribute path to match
    ///
    /// # Returns
    ///
    /// Returns `Some(Match)` if the pattern matches, `None` otherwise.
    ///
    /// # Errors
    ///
    /// Returns an error if pattern compilation fails.
    pub fn matches<'a>(&'a self, input: &'a str) -> RuleResult<Option<RuleMatch<'a>>> {
        let regex = self.compile()?;
        Ok(regex.captures(input).map(|caps| RuleMatch {
            rule: self,
            captures: caps,
        }))
    }

    /// Apply the rule to generate a metric name from captures
    ///
    /// Substitutes `$1`, `$2`, etc. and named groups `$name` with captured values.
    pub fn apply_name(&self, captures: &regex::Captures<'_>) -> String {
        apply_substitution(&self.name, captures)
    }

    /// Apply substitution to labels
    pub fn apply_labels(&self, captures: &regex::Captures<'_>) -> HashMap<String, String> {
        self.labels
            .iter()
            .map(|(k, v)| {
                (
                    apply_substitution(k, captures),
                    apply_substitution(v, captures),
                )
            })
            .collect()
    }

    /// Validate the rule configuration
    ///
    /// Checks that the pattern is valid and the name template is properly formed.
    pub fn validate(&self) -> RuleResult<()> {
        // Validate pattern by compiling it
        self.compile()?;

        // Validate name is not empty
        if self.name.is_empty() {
            return Err(RuleError::ValidationError(
                "Rule name cannot be empty".to_string(),
            ));
        }

        // Validate value factor if present
        if let Some(factor) = self.value_factor {
            if factor.is_nan() || factor.is_infinite() {
                return Err(RuleError::ValidationError(
                    "Value factor must be a finite number".to_string(),
                ));
            }
        }

        Ok(())
    }
}

impl Default for Rule {
    fn default() -> Self {
        Self {
            pattern: String::new(),
            name: String::new(),
            metric_type: MetricType::default(),
            labels: HashMap::new(),
            help: None,
            value: None,
            value_factor: None,
            compiled_pattern: OnceCell::new(),
        }
    }
}

/// Builder for creating Rule instances with fluent API
pub struct RuleBuilder {
    pattern: String,
    name: String,
    metric_type: MetricType,
    labels: HashMap<String, String>,
    help: Option<String>,
    value: Option<String>,
    value_factor: Option<f64>,
}

impl RuleBuilder {
    /// Create a new rule builder
    pub fn new(pattern: impl Into<String>) -> Self {
        Self {
            pattern: pattern.into(),
            name: String::new(),
            metric_type: MetricType::default(),
            labels: HashMap::new(),
            help: None,
            value: None,
            value_factor: None,
        }
    }

    /// Set the metric name
    pub fn name(mut self, name: impl Into<String>) -> Self {
        self.name = name.into();
        self
    }

    /// Set the metric type
    pub fn metric_type(mut self, metric_type: MetricType) -> Self {
        self.metric_type = metric_type;
        self
    }

    /// Add a label
    pub fn label(mut self, key: impl Into<String>, value: impl Into<String>) -> Self {
        self.labels.insert(key.into(), value.into());
        self
    }

    /// Set help text
    pub fn help(mut self, help: impl Into<String>) -> Self {
        self.help = Some(help.into());
        self
    }

    /// Set value expression
    pub fn value(mut self, value: impl Into<String>) -> Self {
        self.value = Some(value.into());
        self
    }

    /// Set value factor
    pub fn value_factor(mut self, factor: f64) -> Self {
        self.value_factor = Some(factor);
        self
    }

    /// Build the rule
    pub fn build(self) -> Rule {
        Rule {
            pattern: self.pattern,
            name: self.name,
            metric_type: self.metric_type,
            labels: self.labels,
            help: self.help,
            value: self.value,
            value_factor: self.value_factor,
            compiled_pattern: OnceCell::new(),
        }
    }
}

/// Result of a successful rule match
pub struct RuleMatch<'a> {
    /// The rule that matched
    pub rule: &'a Rule,
    /// The regex captures from the match
    pub captures: regex::Captures<'a>,
}

impl<'a> RuleMatch<'a> {
    /// Get the full matched string
    pub fn as_str(&self) -> &str {
        self.captures.get(0).map(|m| m.as_str()).unwrap_or("")
    }

    /// Get a capture group by index (1-based)
    pub fn get(&self, index: usize) -> Option<&str> {
        self.captures.get(index).map(|m| m.as_str())
    }

    /// Get a capture group by name
    pub fn name(&self, name: &str) -> Option<&str> {
        self.captures.name(name).map(|m| m.as_str())
    }

    /// Generate the metric name with substitutions applied
    pub fn metric_name(&self) -> String {
        self.rule.apply_name(&self.captures)
    }

    /// Generate labels with substitutions applied
    pub fn labels(&self) -> HashMap<String, String> {
        self.rule.apply_labels(&self.captures)
    }

    /// Get the metric type
    pub fn metric_type(&self) -> MetricType {
        self.rule.metric_type
    }

    /// Get the help text
    pub fn help(&self) -> Option<&str> {
        self.rule.help.as_deref()
    }

    /// Get the value factor
    pub fn value_factor(&self) -> Option<f64> {
        self.rule.value_factor
    }

    /// Get the value expression
    pub fn value(&self) -> Option<&str> {
        self.rule.value.as_deref()
    }
}

/// Collection of transformation rules
///
/// Manages a set of rules for matching and transforming metrics.
/// Supports pre-compilation of all patterns for performance.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RuleSet {
    /// The collection of rules
    rules: Vec<Rule>,
}

impl RuleSet {
    /// Create a new empty rule set
    pub fn new() -> Self {
        Self { rules: Vec::new() }
    }

    /// Create a rule set from a vector of rules
    pub fn from_rules(rules: Vec<Rule>) -> Self {
        Self { rules }
    }

    /// Add a rule to the set
    pub fn add(&mut self, rule: Rule) {
        self.rules.push(rule);
    }

    /// Get the number of rules
    pub fn len(&self) -> usize {
        self.rules.len()
    }

    /// Check if the rule set is empty
    pub fn is_empty(&self) -> bool {
        self.rules.is_empty()
    }

    /// Compile all rule patterns
    ///
    /// This method pre-compiles all regex patterns in the rule set.
    /// It's recommended to call this at startup for better performance.
    ///
    /// # Errors
    ///
    /// Returns an error if any pattern fails to compile.
    pub fn compile_all(&self) -> RuleResult<()> {
        for (index, rule) in self.rules.iter().enumerate() {
            rule.compile().map_err(|e| {
                RuleError::CompilationFailed(format!(
                    "Rule {} (pattern: '{}'): {}",
                    index, rule.pattern, e
                ))
            })?;
        }
        Ok(())
    }

    /// Validate all rules in the set
    ///
    /// Checks that all rules have valid patterns and configurations.
    pub fn validate_all(&self) -> RuleResult<()> {
        for (index, rule) in self.rules.iter().enumerate() {
            rule.validate().map_err(|e| {
                RuleError::ValidationError(format!("Rule {} validation failed: {}", index, e))
            })?;
        }
        Ok(())
    }

    /// Find the first rule that matches the input
    ///
    /// # Arguments
    ///
    /// * `input` - The MBean object name or attribute path to match
    ///
    /// # Returns
    ///
    /// Returns `Some(RuleMatch)` for the first matching rule, `None` if no rules match.
    pub fn find_match<'a>(&'a self, input: &'a str) -> RuleResult<Option<RuleMatch<'a>>> {
        for rule in &self.rules {
            if let Some(m) = rule.matches(input)? {
                return Ok(Some(m));
            }
        }
        Ok(None)
    }

    /// Find all rules that match the input
    ///
    /// # Arguments
    ///
    /// * `input` - The MBean object name or attribute path to match
    ///
    /// # Returns
    ///
    /// Returns a vector of all matching rules with their captures.
    pub fn find_all_matches<'a>(&'a self, input: &'a str) -> RuleResult<Vec<RuleMatch<'a>>> {
        let mut matches = Vec::new();
        for rule in &self.rules {
            if let Some(m) = rule.matches(input)? {
                matches.push(m);
            }
        }
        Ok(matches)
    }

    /// Iterate over all rules
    pub fn iter(&self) -> impl Iterator<Item = &Rule> {
        self.rules.iter()
    }

    /// Get a reference to the underlying rules vector
    pub fn rules(&self) -> &[Rule] {
        &self.rules
    }

    /// Get a rule by index
    pub fn get(&self, index: usize) -> Option<&Rule> {
        self.rules.get(index)
    }
}

impl IntoIterator for RuleSet {
    type Item = Rule;
    type IntoIter = std::vec::IntoIter<Rule>;

    fn into_iter(self) -> Self::IntoIter {
        self.rules.into_iter()
    }
}

impl<'a> IntoIterator for &'a RuleSet {
    type Item = &'a Rule;
    type IntoIter = std::slice::Iter<'a, Rule>;

    fn into_iter(self) -> Self::IntoIter {
        self.rules.iter()
    }
}

impl FromIterator<Rule> for RuleSet {
    fn from_iter<I: IntoIterator<Item = Rule>>(iter: I) -> Self {
        Self {
            rules: iter.into_iter().collect(),
        }
    }
}

/// Convert Java regex syntax to Rust regex syntax
///
/// Handles common differences between Java and Rust regex:
/// - Named groups: `(?<name>...)` → `(?P<name>...)`
/// - Possessive quantifiers: `++`, `*+`, `?+` → `+`, `*`, `?` (with warning)
/// - Atomic groups: `(?>...)` → Error (not supported)
///
/// # Arguments
///
/// * `pattern` - The Java regex pattern to convert
///
/// # Returns
///
/// Returns the converted Rust regex pattern.
///
/// # Errors
///
/// Returns `RuleError::UnsupportedJavaFeature` for unsupported features.
pub fn convert_java_regex(pattern: &str) -> RuleResult<String> {
    let mut result = String::with_capacity(pattern.len() + 16);
    let mut chars = pattern.chars().peekable();

    while let Some(c) = chars.next() {
        match c {
            '(' => {
                if chars.peek() == Some(&'?') {
                    chars.next(); // consume '?'
                    match chars.peek() {
                        Some('<') => {
                            // Check if it's a named group (?<name>...)
                            chars.next(); // consume '<'

                            // Check for lookbehind assertions
                            match chars.peek() {
                                Some('=') => {
                                    // Lookbehind assertion (?<=...) - not supported in Rust regex
                                    return Err(RuleError::UnsupportedJavaFeature {
                                        pattern: pattern.to_string(),
                                        feature: "positive lookbehind assertions (?<=...)"
                                            .to_string(),
                                    });
                                }
                                Some('!') => {
                                    // Negative lookbehind assertion (?<!...) - not supported in Rust regex
                                    return Err(RuleError::UnsupportedJavaFeature {
                                        pattern: pattern.to_string(),
                                        feature: "negative lookbehind assertions (?<!...)"
                                            .to_string(),
                                    });
                                }
                                _ => {
                                    // Named group - convert to Rust syntax
                                    result.push_str("(?P<");
                                }
                            }
                        }
                        Some('>') => {
                            // Atomic group (?>...) - not supported in Rust regex
                            return Err(RuleError::UnsupportedJavaFeature {
                                pattern: pattern.to_string(),
                                feature: "atomic groups (?>...)".to_string(),
                            });
                        }
                        Some('=') => {
                            // Positive lookahead (?=...) - not supported in Rust regex
                            return Err(RuleError::UnsupportedJavaFeature {
                                pattern: pattern.to_string(),
                                feature: "positive lookahead assertions (?=...)".to_string(),
                            });
                        }
                        Some('!') => {
                            // Negative lookahead (?!...) - not supported in Rust regex
                            return Err(RuleError::UnsupportedJavaFeature {
                                pattern: pattern.to_string(),
                                feature: "negative lookahead assertions (?!...)".to_string(),
                            });
                        }
                        _ => {
                            // Other special groups like (?:...)
                            result.push_str("(?");
                        }
                    }
                } else {
                    result.push('(');
                }
            }
            '+' | '*' | '?' => {
                result.push(c);
                // Check for possessive quantifier
                if chars.peek() == Some(&'+') {
                    chars.next(); // consume the extra '+'
                    tracing::warn!(
                        pattern = %pattern,
                        "Possessive quantifier '{}+' converted to '{}' - behavior may differ",
                        c, c
                    );
                }
            }
            '\\' => {
                // Preserve escape sequences
                result.push(c);
                if let Some(escaped) = chars.next() {
                    result.push(escaped);
                }
            }
            _ => {
                result.push(c);
            }
        }
    }

    Ok(result)
}

/// Apply capture group substitution to a template string
///
/// Replaces `$1`, `$2`, etc. with the corresponding capture group values.
/// Also supports named groups via `$name` syntax.
fn apply_substitution(template: &str, captures: &regex::Captures<'_>) -> String {
    let mut result = String::with_capacity(template.len());
    let mut chars = template.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '$' {
            // Check what follows the $
            match chars.peek() {
                Some(&first) if first.is_ascii_digit() => {
                    // Numeric group reference ($1, $2, $12, etc.)
                    let mut group_num = String::new();
                    while let Some(&next) = chars.peek() {
                        if next.is_ascii_digit() {
                            group_num.push(next);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if let Ok(index) = group_num.parse::<usize>() {
                        if let Some(m) = captures.get(index) {
                            result.push_str(m.as_str());
                        }
                        // If group doesn't exist, substitute with empty string
                    }
                }
                Some(&first) if first.is_alphabetic() => {
                    // Named group reference ($name)
                    // Note: We only accept letters and digits after the first letter,
                    // NOT underscores. This allows patterns like "$type_$attr" to work
                    // correctly (where $type is substituted, then _, then $attr).
                    let mut group_name = String::new();
                    while let Some(&next) = chars.peek() {
                        if next.is_alphanumeric() {
                            group_name.push(next);
                            chars.next();
                        } else {
                            break;
                        }
                    }
                    if let Some(m) = captures.name(&group_name) {
                        result.push_str(m.as_str());
                    }
                    // If group doesn't exist, substitute with empty string
                }
                _ => {
                    // Literal $ (at end of string or followed by non-identifier char)
                    result.push('$');
                }
            }
        } else {
            result.push(c);
        }
    }

    result
}

#[cfg(test)]
mod tests {
    use super::*;

    // ==========================================================================
    // MetricType tests
    // ==========================================================================

    #[test]
    fn test_metric_type_default() {
        assert_eq!(MetricType::default(), MetricType::Untyped);
    }

    #[test]
    fn test_metric_type_as_str() {
        assert_eq!(MetricType::Gauge.as_str(), "gauge");
        assert_eq!(MetricType::Counter.as_str(), "counter");
        assert_eq!(MetricType::Untyped.as_str(), "untyped");
    }

    #[test]
    fn test_metric_type_display() {
        assert_eq!(format!("{}", MetricType::Gauge), "gauge");
        assert_eq!(format!("{}", MetricType::Counter), "counter");
        assert_eq!(format!("{}", MetricType::Untyped), "untyped");
    }

    #[test]
    fn test_metric_type_serialize() {
        let gauge = MetricType::Gauge;
        let json = serde_json::to_string(&gauge).unwrap();
        assert_eq!(json, "\"gauge\"");

        let counter = MetricType::Counter;
        let json = serde_json::to_string(&counter).unwrap();
        assert_eq!(json, "\"counter\"");
    }

    #[test]
    fn test_metric_type_deserialize_lowercase() {
        let gauge: MetricType = serde_json::from_str("\"gauge\"").unwrap();
        assert_eq!(gauge, MetricType::Gauge);

        let counter: MetricType = serde_json::from_str("\"counter\"").unwrap();
        assert_eq!(counter, MetricType::Counter);

        let untyped: MetricType = serde_json::from_str("\"untyped\"").unwrap();
        assert_eq!(untyped, MetricType::Untyped);
    }

    #[test]
    fn test_metric_type_deserialize_case_insensitive() {
        let gauge: MetricType = serde_json::from_str("\"GAUGE\"").unwrap();
        assert_eq!(gauge, MetricType::Gauge);

        let gauge: MetricType = serde_json::from_str("\"Gauge\"").unwrap();
        assert_eq!(gauge, MetricType::Gauge);

        let counter: MetricType = serde_json::from_str("\"COUNTER\"").unwrap();
        assert_eq!(counter, MetricType::Counter);

        let counter: MetricType = serde_json::from_str("\"Counter\"").unwrap();
        assert_eq!(counter, MetricType::Counter);
    }

    #[test]
    fn test_metric_type_deserialize_invalid() {
        let result: Result<MetricType, _> = serde_json::from_str("\"invalid\"");
        assert!(result.is_err());
        let err = result.unwrap_err().to_string();
        assert!(err.contains("unknown metric type"));
    }

    // ==========================================================================
    // Rule tests
    // ==========================================================================

    #[test]
    fn test_rule_new() {
        let rule = Rule::new(
            r"java\.lang<type=Memory><HeapMemoryUsage>(\w+)",
            "jvm_memory_heap_$1_bytes",
            MetricType::Gauge,
        );
        assert_eq!(
            rule.pattern,
            r"java\.lang<type=Memory><HeapMemoryUsage>(\w+)"
        );
        assert_eq!(rule.name, "jvm_memory_heap_$1_bytes");
        assert_eq!(rule.metric_type, MetricType::Gauge);
    }

    #[test]
    fn test_rule_builder() {
        let rule = Rule::builder(r"java\.lang<type=Memory>")
            .name("jvm_memory")
            .metric_type(MetricType::Gauge)
            .label("area", "heap")
            .help("JVM memory usage")
            .value_factor(0.001)
            .build();

        assert_eq!(rule.pattern, r"java\.lang<type=Memory>");
        assert_eq!(rule.name, "jvm_memory");
        assert_eq!(rule.metric_type, MetricType::Gauge);
        assert_eq!(rule.labels.get("area"), Some(&"heap".to_string()));
        assert_eq!(rule.help, Some("JVM memory usage".to_string()));
        assert_eq!(rule.value_factor, Some(0.001));
    }

    #[test]
    fn test_rule_with_methods() {
        let rule = Rule::new("pattern", "name", MetricType::Gauge)
            .with_label("key", "value")
            .with_help("help text")
            .with_value_factor(2.0);

        assert_eq!(rule.labels.get("key"), Some(&"value".to_string()));
        assert_eq!(rule.help, Some("help text".to_string()));
        assert_eq!(rule.value_factor, Some(2.0));
    }

    #[test]
    fn test_rule_compile() {
        let rule = Rule::new(r"test(\d+)", "metric_$1", MetricType::Gauge);
        let regex = rule.compile().unwrap();
        assert!(regex.is_match("test123"));
        assert!(!regex.is_match("testABC"));
    }

    #[test]
    fn test_rule_compile_invalid() {
        let rule = Rule::new(r"test[", "metric", MetricType::Gauge);
        let result = rule.compile();
        assert!(result.is_err());
        match result {
            Err(RuleError::InvalidPattern { pattern, .. }) => {
                assert_eq!(pattern, "test[");
            }
            _ => panic!("Expected InvalidPattern error"),
        }
    }

    #[test]
    fn test_rule_matches() {
        let rule = Rule::new(
            r"java\.lang<type=(\w+)><(\w+)>(\w+)",
            "jvm_$1_$2_$3",
            MetricType::Gauge,
        );

        let result = rule
            .matches("java.lang<type=Memory><HeapMemoryUsage>used")
            .unwrap();
        assert!(result.is_some());

        let m = result.unwrap();
        assert_eq!(m.get(1), Some("Memory"));
        assert_eq!(m.get(2), Some("HeapMemoryUsage"));
        assert_eq!(m.get(3), Some("used"));
    }

    #[test]
    fn test_rule_matches_no_match() {
        let rule = Rule::new(r"java\.lang", "metric", MetricType::Gauge);
        let result = rule.matches("com.example").unwrap();
        assert!(result.is_none());
    }

    #[test]
    fn test_rule_apply_name() {
        let rule = Rule::new(
            r"java\.lang<type=(\w+)><(\w+)>(\w+)",
            "jvm_$1_$2_$3_bytes",
            MetricType::Gauge,
        );

        let regex = rule.compile().unwrap();
        let caps = regex
            .captures("java.lang<type=Memory><HeapMemoryUsage>used")
            .unwrap();
        let name = rule.apply_name(&caps);

        assert_eq!(name, "jvm_Memory_HeapMemoryUsage_used_bytes");
    }

    #[test]
    fn test_rule_apply_labels() {
        let rule = Rule::new(r"java\.lang<type=(\w+)>", "metric", MetricType::Gauge)
            .with_label("type", "$1")
            .with_label("static", "value");

        let regex = rule.compile().unwrap();
        let caps = regex.captures("java.lang<type=Memory>").unwrap();
        let labels = rule.apply_labels(&caps);

        assert_eq!(labels.get("type"), Some(&"Memory".to_string()));
        assert_eq!(labels.get("static"), Some(&"value".to_string()));
    }

    #[test]
    fn test_rule_validate_empty_name() {
        let rule = Rule::new(r"pattern", "", MetricType::Gauge);
        let result = rule.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_rule_validate_invalid_value_factor() {
        let rule = Rule::new(r"pattern", "name", MetricType::Gauge).with_value_factor(f64::NAN);
        let result = rule.validate();
        assert!(result.is_err());

        let rule =
            Rule::new(r"pattern", "name", MetricType::Gauge).with_value_factor(f64::INFINITY);
        let result = rule.validate();
        assert!(result.is_err());
    }

    #[test]
    fn test_rule_serialize_deserialize() {
        let rule = Rule::builder(r"java\.lang<type=Memory>")
            .name("jvm_memory")
            .metric_type(MetricType::Gauge)
            .label("area", "heap")
            .help("Memory usage")
            .value_factor(0.001)
            .build();

        let yaml = serde_yaml::to_string(&rule).unwrap();
        let deserialized: Rule = serde_yaml::from_str(&yaml).unwrap();

        assert_eq!(deserialized.pattern, rule.pattern);
        assert_eq!(deserialized.name, rule.name);
        assert_eq!(deserialized.metric_type, rule.metric_type);
        assert_eq!(deserialized.labels, rule.labels);
        assert_eq!(deserialized.help, rule.help);
        assert_eq!(deserialized.value_factor, rule.value_factor);
    }

    // ==========================================================================
    // RuleMatch tests
    // ==========================================================================

    #[test]
    fn test_rule_match_methods() {
        let rule = Rule::builder(r"java\.lang<type=(?P<type>\w+)><(\w+)>")
            .name("jvm_$type_$2")
            .metric_type(MetricType::Gauge)
            .help("Test help")
            .value_factor(0.5)
            .label("type", "$type")
            .build();

        let m = rule
            .matches("java.lang<type=Memory><HeapMemoryUsage>")
            .unwrap()
            .unwrap();

        assert_eq!(m.as_str(), "java.lang<type=Memory><HeapMemoryUsage>");
        assert_eq!(m.get(1), Some("Memory"));
        assert_eq!(m.get(2), Some("HeapMemoryUsage"));
        assert_eq!(m.name("type"), Some("Memory"));
        assert_eq!(m.metric_name(), "jvm_Memory_HeapMemoryUsage");
        assert_eq!(m.metric_type(), MetricType::Gauge);
        assert_eq!(m.help(), Some("Test help"));
        assert_eq!(m.value_factor(), Some(0.5));

        let labels = m.labels();
        assert_eq!(labels.get("type"), Some(&"Memory".to_string()));
    }

    // ==========================================================================
    // RuleSet tests
    // ==========================================================================

    #[test]
    fn test_ruleset_new() {
        let ruleset = RuleSet::new();
        assert!(ruleset.is_empty());
        assert_eq!(ruleset.len(), 0);
    }

    #[test]
    fn test_ruleset_from_rules() {
        let rules = vec![
            Rule::new("pattern1", "name1", MetricType::Gauge),
            Rule::new("pattern2", "name2", MetricType::Counter),
        ];
        let ruleset = RuleSet::from_rules(rules);
        assert_eq!(ruleset.len(), 2);
    }

    #[test]
    fn test_ruleset_add() {
        let mut ruleset = RuleSet::new();
        ruleset.add(Rule::new("pattern", "name", MetricType::Gauge));
        assert_eq!(ruleset.len(), 1);
    }

    #[test]
    fn test_ruleset_compile_all() {
        let ruleset = RuleSet::from_rules(vec![
            Rule::new(r"java\.lang", "jvm", MetricType::Gauge),
            Rule::new(r"com\.example", "app", MetricType::Counter),
        ]);
        assert!(ruleset.compile_all().is_ok());
    }

    #[test]
    fn test_ruleset_compile_all_invalid() {
        let ruleset = RuleSet::from_rules(vec![
            Rule::new(r"valid", "name", MetricType::Gauge),
            Rule::new(r"invalid[", "name", MetricType::Gauge),
        ]);
        let result = ruleset.compile_all();
        assert!(result.is_err());
    }

    #[test]
    fn test_ruleset_find_match() {
        let ruleset = RuleSet::from_rules(vec![
            Rule::new(r"java\.lang", "jvm", MetricType::Gauge),
            Rule::new(r"com\.example", "app", MetricType::Counter),
        ]);

        let m = ruleset.find_match("java.lang<type=Memory>").unwrap();
        assert!(m.is_some());
        assert_eq!(m.unwrap().rule.name, "jvm");

        let m = ruleset.find_match("com.example.Service").unwrap();
        assert!(m.is_some());
        assert_eq!(m.unwrap().rule.name, "app");

        let m = ruleset.find_match("other.package").unwrap();
        assert!(m.is_none());
    }

    #[test]
    fn test_ruleset_find_all_matches() {
        let ruleset = RuleSet::from_rules(vec![
            Rule::new(r"java", "java_metric", MetricType::Gauge),
            Rule::new(r"java\.lang", "jvm_metric", MetricType::Gauge),
        ]);

        let matches = ruleset.find_all_matches("java.lang<type=Memory>").unwrap();
        assert_eq!(matches.len(), 2);
    }

    #[test]
    fn test_ruleset_iter() {
        let ruleset = RuleSet::from_rules(vec![
            Rule::new("p1", "n1", MetricType::Gauge),
            Rule::new("p2", "n2", MetricType::Counter),
        ]);

        let names: Vec<_> = ruleset.iter().map(|r| r.name.as_str()).collect();
        assert_eq!(names, vec!["n1", "n2"]);
    }

    #[test]
    fn test_ruleset_into_iter() {
        let ruleset = RuleSet::from_rules(vec![
            Rule::new("p1", "n1", MetricType::Gauge),
            Rule::new("p2", "n2", MetricType::Counter),
        ]);

        let names: Vec<_> = ruleset.into_iter().map(|r| r.name).collect();
        assert_eq!(names, vec!["n1", "n2"]);
    }

    #[test]
    fn test_ruleset_from_iter() {
        let rules = vec![
            Rule::new("p1", "n1", MetricType::Gauge),
            Rule::new("p2", "n2", MetricType::Counter),
        ];

        let ruleset: RuleSet = rules.into_iter().collect();
        assert_eq!(ruleset.len(), 2);
    }

    #[test]
    fn test_ruleset_get() {
        let ruleset = RuleSet::from_rules(vec![
            Rule::new("p1", "n1", MetricType::Gauge),
            Rule::new("p2", "n2", MetricType::Counter),
        ]);

        assert_eq!(ruleset.get(0).map(|r| &r.name), Some(&"n1".to_string()));
        assert_eq!(ruleset.get(1).map(|r| &r.name), Some(&"n2".to_string()));
        assert!(ruleset.get(2).is_none());
    }

    // ==========================================================================
    // Java regex conversion tests
    // ==========================================================================

    #[test]
    fn test_convert_java_regex_named_group() {
        let result = convert_java_regex(r"(?<name>\w+)").unwrap();
        assert_eq!(result, r"(?P<name>\w+)");
    }

    #[test]
    fn test_convert_java_regex_multiple_named_groups() {
        let result = convert_java_regex(r"(?<type>\w+)<(?<attr>\w+)>").unwrap();
        assert_eq!(result, r"(?P<type>\w+)<(?P<attr>\w+)>");
    }

    #[test]
    fn test_convert_java_regex_non_capturing_group() {
        let result = convert_java_regex(r"(?:abc)+").unwrap();
        assert_eq!(result, r"(?:abc)+");
    }

    #[test]
    fn test_convert_java_regex_lookahead() {
        // Positive lookahead - not supported in Rust regex
        let result = convert_java_regex(r"foo(?=bar)");
        assert!(result.is_err());
        match result {
            Err(RuleError::UnsupportedJavaFeature { feature, .. }) => {
                assert!(feature.contains("positive lookahead"));
            }
            _ => panic!("Expected UnsupportedJavaFeature error"),
        }

        // Negative lookahead - not supported in Rust regex
        let result = convert_java_regex(r"foo(?!bar)");
        assert!(result.is_err());
        match result {
            Err(RuleError::UnsupportedJavaFeature { feature, .. }) => {
                assert!(feature.contains("negative lookahead"));
            }
            _ => panic!("Expected UnsupportedJavaFeature error"),
        }
    }

    #[test]
    fn test_convert_java_regex_lookbehind() {
        // Positive lookbehind - not supported in Rust regex
        let result = convert_java_regex(r"(?<=foo)bar");
        assert!(result.is_err());
        match result {
            Err(RuleError::UnsupportedJavaFeature { feature, .. }) => {
                assert!(feature.contains("positive lookbehind"));
            }
            _ => panic!("Expected UnsupportedJavaFeature error"),
        }

        // Negative lookbehind - not supported in Rust regex
        let result = convert_java_regex(r"(?<!foo)bar");
        assert!(result.is_err());
        match result {
            Err(RuleError::UnsupportedJavaFeature { feature, .. }) => {
                assert!(feature.contains("negative lookbehind"));
            }
            _ => panic!("Expected UnsupportedJavaFeature error"),
        }
    }

    #[test]
    fn test_convert_java_regex_possessive_quantifiers() {
        // Possessive quantifiers are converted with warning
        let result = convert_java_regex(r"a++").unwrap();
        assert_eq!(result, r"a+");

        let result = convert_java_regex(r"a*+").unwrap();
        assert_eq!(result, r"a*");

        let result = convert_java_regex(r"a?+").unwrap();
        assert_eq!(result, r"a?");
    }

    #[test]
    fn test_convert_java_regex_atomic_group() {
        let result = convert_java_regex(r"(?>abc)");
        assert!(result.is_err());
        match result {
            Err(RuleError::UnsupportedJavaFeature { feature, .. }) => {
                assert!(feature.contains("atomic group"));
            }
            _ => panic!("Expected UnsupportedJavaFeature error"),
        }
    }

    #[test]
    fn test_convert_java_regex_escape_sequences() {
        let result = convert_java_regex(r"\\d+\.\w+").unwrap();
        assert_eq!(result, r"\\d+\.\w+");
    }

    #[test]
    fn test_convert_java_regex_complex_pattern() {
        let result =
            convert_java_regex(r"java\.lang<type=(?<type>\w+)><(?<attr>\w+)>(?:\w+)").unwrap();
        assert_eq!(
            result,
            r"java\.lang<type=(?P<type>\w+)><(?P<attr>\w+)>(?:\w+)"
        );
    }

    // ==========================================================================
    // Substitution tests
    // ==========================================================================

    #[test]
    fn test_apply_substitution_numeric() {
        let regex = Regex::new(r"(\w+)<(\w+)>").unwrap();
        let caps = regex.captures("Memory<HeapUsage>").unwrap();

        let result = apply_substitution("jvm_$1_$2", &caps);
        assert_eq!(result, "jvm_Memory_HeapUsage");
    }

    #[test]
    fn test_apply_substitution_named() {
        let regex = Regex::new(r"(?P<type>\w+)<(?P<attr>\w+)>").unwrap();
        let caps = regex.captures("Memory<HeapUsage>").unwrap();

        let result = apply_substitution("jvm_$type_$attr", &caps);
        assert_eq!(result, "jvm_Memory_HeapUsage");
    }

    #[test]
    fn test_apply_substitution_mixed() {
        let regex = Regex::new(r"(?P<type>\w+)<(\w+)>").unwrap();
        let caps = regex.captures("Memory<HeapUsage>").unwrap();

        let result = apply_substitution("jvm_$type_$2", &caps);
        assert_eq!(result, "jvm_Memory_HeapUsage");
    }

    #[test]
    fn test_apply_substitution_missing_group() {
        let regex = Regex::new(r"(\w+)").unwrap();
        let caps = regex.captures("Memory").unwrap();

        // $2 doesn't exist, should be replaced with empty string
        let result = apply_substitution("jvm_$1_$2", &caps);
        assert_eq!(result, "jvm_Memory_");
    }

    #[test]
    fn test_apply_substitution_literal_dollar() {
        let regex = Regex::new(r"(\w+)").unwrap();
        let caps = regex.captures("Memory").unwrap();

        // $ at end is preserved
        let result = apply_substitution("price_$1_$", &caps);
        assert_eq!(result, "price_Memory_$");
    }

    // ==========================================================================
    // Integration tests
    // ==========================================================================

    #[test]
    fn test_jmx_exporter_pattern_compatibility() {
        // Test pattern similar to what jmx_exporter uses
        let rule = Rule::builder(r"java\.lang<type=Memory><HeapMemoryUsage>(\w+)")
            .name("jvm_memory_heap_$1_bytes")
            .metric_type(MetricType::Gauge)
            .help("JVM heap memory usage")
            .label("area", "heap")
            .build();

        let m = rule
            .matches("java.lang<type=Memory><HeapMemoryUsage>used")
            .unwrap()
            .unwrap();
        assert_eq!(m.metric_name(), "jvm_memory_heap_used_bytes");
        assert_eq!(m.help(), Some("JVM heap memory usage"));

        let labels = m.labels();
        assert_eq!(labels.get("area"), Some(&"heap".to_string()));
    }

    #[test]
    fn test_gc_pattern() {
        let rule = Rule::builder(r"java\.lang<type=GarbageCollector,name=(?P<gc>\w+)><(\w+)>")
            .name("jvm_gc_$gc_$2")
            .metric_type(MetricType::Counter)
            .label("gc", "$gc")
            .build();

        let m = rule
            .matches("java.lang<type=GarbageCollector,name=G1YoungGen><CollectionCount>")
            .unwrap()
            .unwrap();

        assert_eq!(m.metric_name(), "jvm_gc_G1YoungGen_CollectionCount");
        assert_eq!(m.name("gc"), Some("G1YoungGen"));
        assert_eq!(m.get(2), Some("CollectionCount"));
    }

    #[test]
    fn test_thread_pattern() {
        let rule = Rule::new(
            r"java\.lang<type=Threading><(\w+)>",
            "jvm_threads_$1",
            MetricType::Gauge,
        );

        let m = rule
            .matches("java.lang<type=Threading><ThreadCount>")
            .unwrap()
            .unwrap();
        assert_eq!(m.metric_name(), "jvm_threads_ThreadCount");
    }

    #[test]
    fn test_yaml_config_roundtrip() {
        let yaml = r#"
pattern: "java\\.lang<type=Memory>"
name: jvm_memory
type: gauge
labels:
  area: heap
help: "JVM memory usage"
valueFactor: 0.001
"#;

        let rule: Rule = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(rule.pattern, r"java\.lang<type=Memory>");
        assert_eq!(rule.name, "jvm_memory");
        assert_eq!(rule.metric_type, MetricType::Gauge);
        assert_eq!(rule.labels.get("area"), Some(&"heap".to_string()));
        assert_eq!(rule.help, Some("JVM memory usage".to_string()));
        assert_eq!(rule.value_factor, Some(0.001));
    }

    #[test]
    fn test_ruleset_yaml_config() {
        let yaml = r#"
rules:
  - pattern: "java\\.lang<type=Memory>"
    name: jvm_memory
    type: gauge
  - pattern: "java\\.lang<type=Threading>"
    name: jvm_threads
    type: gauge
"#;

        #[derive(Deserialize)]
        struct Config {
            rules: Vec<Rule>,
        }

        let config: Config = serde_yaml::from_str(yaml).unwrap();
        let ruleset = RuleSet::from_rules(config.rules);

        assert_eq!(ruleset.len(), 2);
        assert!(ruleset.compile_all().is_ok());
    }
}
