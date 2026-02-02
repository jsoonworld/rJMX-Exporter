//! Configuration management for rJMX-Exporter
//!
//! Handles loading and validating configuration from YAML files.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    /// Error reading the configuration file
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    /// Error parsing the configuration file
    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_yaml::Error),

    /// Configuration validation error
    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

/// Main configuration structure
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct Config {
    /// Jolokia endpoint configuration
    #[serde(default)]
    pub jolokia: JolokiaConfig,

    /// HTTP server configuration
    #[serde(default)]
    pub server: ServerConfig,

    /// Metric transformation rules
    #[serde(default)]
    pub rules: Vec<Rule>,

    /// Convert metric names to lowercase (jmx_exporter compatible)
    #[serde(rename = "lowercaseOutputName", default)]
    pub lowercase_output_name: bool,

    /// Convert label names to lowercase (jmx_exporter compatible)
    #[serde(rename = "lowercaseOutputLabelNames", default)]
    pub lowercase_output_label_names: bool,

    /// MBean whitelist patterns (glob patterns, jmx_exporter compatible)
    #[serde(rename = "whitelistObjectNames", default)]
    pub whitelist_object_names: Vec<String>,

    /// MBean blacklist patterns (glob patterns, jmx_exporter compatible)
    #[serde(rename = "blacklistObjectNames", default)]
    pub blacklist_object_names: Vec<String>,
}

/// Jolokia endpoint configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct JolokiaConfig {
    /// Jolokia endpoint URL
    #[serde(default = "default_jolokia_url")]
    pub url: String,

    /// Optional username for basic auth
    pub username: Option<String>,

    /// Optional password for basic auth
    pub password: Option<String>,

    /// Request timeout in milliseconds
    #[serde(default = "default_timeout")]
    pub timeout_ms: u64,
}

/// HTTP server configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServerConfig {
    /// Server port
    #[serde(default = "default_port")]
    pub port: u16,

    /// Metrics endpoint path
    #[serde(default = "default_metrics_path")]
    pub path: String,

    /// Server bind address (IP address or "localhost")
    ///
    /// Supported values:
    /// - IP addresses: "0.0.0.0", "127.0.0.1", "::1", etc.
    /// - "localhost" (maps to 127.0.0.1)
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
}

/// Metric transformation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// MBean pattern to match (regex)
    pub pattern: String,

    /// Prometheus metric name (supports $1, $2, etc. for capture groups)
    pub name: String,

    /// Metric type (gauge, counter, untyped)
    #[serde(default = "default_metric_type")]
    pub r#type: String,

    /// Optional help text for the metric
    pub help: Option<String>,

    /// Optional static labels to add to the metric
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,

    /// Value extraction expression (jmx_exporter compatible)
    /// Supports attribute references like "$1" for capture groups
    pub value: Option<String>,

    /// Value multiplication factor (jmx_exporter compatible)
    /// The extracted value will be multiplied by this factor
    #[serde(rename = "valueFactor", default)]
    pub value_factor: Option<f64>,
}

// Default value functions
fn default_jolokia_url() -> String {
    "http://localhost:8778/jolokia".to_string()
}

fn default_timeout() -> u64 {
    5000
}

fn default_port() -> u16 {
    9090
}

fn default_metrics_path() -> String {
    "/metrics".to_string()
}

fn default_bind_address() -> String {
    "0.0.0.0".to_string()
}

fn default_metric_type() -> String {
    "untyped".to_string()
}

impl Default for JolokiaConfig {
    fn default() -> Self {
        Self {
            url: default_jolokia_url(),
            username: None,
            password: None,
            timeout_ms: default_timeout(),
        }
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            port: default_port(),
            path: default_metrics_path(),
            bind_address: default_bind_address(),
        }
    }
}

impl Config {
    /// Load configuration from a YAML file
    ///
    /// # Arguments
    /// * `path` - Path to the configuration file
    ///
    /// # Errors
    /// Returns an error if the file cannot be read or parsed
    ///
    /// # Note
    /// - If the file doesn't exist, returns `ConfigError::ReadError`
    /// - Use `Config::load_or_default()` if you want fallback to defaults
    pub fn load<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();
        let contents = std::fs::read_to_string(path)?;
        let config: Config = serde_yaml::from_str(&contents)?;
        config.validate()?;
        Ok(config)
    }

    /// Load configuration from a YAML file, falling back to defaults if not found
    ///
    /// Use this for optional configuration files (e.g., when running without explicit config)
    pub fn load_or_default<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let path = path.as_ref();

        match std::fs::read_to_string(path) {
            Ok(contents) => {
                let config: Config = serde_yaml::from_str(&contents)?;
                config.validate()?;
                Ok(config)
            }
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {
                tracing::warn!(
                    path = %path.display(),
                    "Config file not found, using defaults"
                );
                Ok(Self::default())
            }
            Err(e) => Err(ConfigError::ReadError(e)),
        }
    }

    /// Validate the configuration
    ///
    /// Note: Port validation is intentionally NOT done here because CLI arguments
    /// may override the port value. Port validation should be done after all
    /// overrides are applied (see main.rs).
    fn validate(&self) -> Result<(), ConfigError> {
        if !self.server.path.starts_with('/') {
            return Err(ConfigError::ValidationError(
                "Metrics path must start with '/'".to_string(),
            ));
        }

        if self.server.path == "/" || self.server.path == "/health" {
            return Err(ConfigError::ValidationError(
                "Metrics path must not conflict with '/' or '/health'".to_string(),
            ));
        }

        // Validate rule patterns are valid regex
        for (idx, rule) in self.rules.iter().enumerate() {
            // Basic regex validation - full validation happens in transformer
            if rule.pattern.is_empty() {
                return Err(ConfigError::ValidationError(format!(
                    "Rule {} has empty pattern",
                    idx
                )));
            }
        }

        Ok(())
    }

    // Convert config rules to transformer RuleSet
    //
    // Note: Requires transformer module - implement when transformer is complete
    // pub fn to_ruleset(&self) -> crate::transformer::RuleSet {
    //     todo!("Implement when transformer module is complete")
    // }

    /// Validate the final port value
    ///
    /// Call this after applying CLI overrides to ensure the port is valid.
    pub fn validate_port(port: u16) -> Result<(), ConfigError> {
        if port == 0 {
            return Err(ConfigError::ValidationError(
                "Server port must be greater than 0".to_string(),
            ));
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_config() {
        let config = Config::default();
        assert_eq!(config.server.port, 9090);
        assert_eq!(config.server.path, "/metrics");
    }

    #[test]
    fn test_config_validation_path() {
        let mut config = Config::default();
        config.server.path = "no-slash".to_string();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_port_validation() {
        assert!(Config::validate_port(0).is_err());
        assert!(Config::validate_port(8080).is_ok());
        assert!(Config::validate_port(9090).is_ok());
    }

    #[test]
    fn test_rule_pattern_validation() {
        let mut config = Config::default();
        config.rules.push(Rule {
            pattern: String::new(),
            name: "test_metric".to_string(),
            r#type: "gauge".to_string(),
            help: None,
            labels: std::collections::HashMap::new(),
            value: None,
            value_factor: None,
        });
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_jmx_exporter_compat_fields() {
        let yaml = r#"
lowercaseOutputName: true
lowercaseOutputLabelNames: true
whitelistObjectNames:
  - "java.lang:*"
  - "com.example:*"
blacklistObjectNames:
  - "java.lang:type=MemoryPool,*"
rules:
  - pattern: "java.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
    value: "$1"
    valueFactor: 1.0
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.lowercase_output_name);
        assert!(config.lowercase_output_label_names);
        assert_eq!(config.whitelist_object_names.len(), 2);
        assert_eq!(config.blacklist_object_names.len(), 1);
        assert_eq!(config.rules.len(), 1);
        assert_eq!(config.rules[0].value, Some("$1".to_string()));
        assert_eq!(config.rules[0].value_factor, Some(1.0));
    }
}
