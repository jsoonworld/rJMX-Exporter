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

    /// TLS configuration for HTTPS support
    #[serde(default)]
    pub tls: TlsConfig,
}

/// TLS configuration for HTTPS support
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct TlsConfig {
    /// Enable TLS/HTTPS (default: false)
    #[serde(default)]
    pub enabled: bool,

    /// Path to the TLS certificate file (PEM format)
    #[serde(default)]
    pub cert_file: Option<String>,

    /// Path to the TLS private key file (PEM format)
    #[serde(default)]
    pub key_file: Option<String>,
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
            tls: TlsConfig::default(),
        }
    }
}

/// Configuration overrides from CLI arguments and environment variables
///
/// These are applied on top of config file values.
/// Fields are Option to indicate "no override" vs "explicit override".
///
/// The precedence order is:
/// 1. CLI arguments (highest priority)
/// 2. Environment variables
/// 3. Configuration file
/// 4. Default values (lowest priority)
#[derive(Debug, Clone, Default)]
pub struct ConfigOverrides {
    /// Server port override
    pub port: Option<u16>,
    /// Server bind address override
    pub bind_address: Option<String>,
    /// Metrics endpoint path override
    pub metrics_path: Option<String>,
    /// Jolokia URL override
    pub jolokia_url: Option<String>,
    /// Jolokia timeout override (milliseconds)
    pub jolokia_timeout: Option<u64>,
    /// Jolokia username override
    pub username: Option<String>,
    /// Jolokia password override
    pub password: Option<String>,
    /// TLS enabled override
    pub tls_enabled: Option<bool>,
    /// TLS certificate file path override
    pub tls_cert_file: Option<String>,
    /// TLS private key file path override
    pub tls_key_file: Option<String>,
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

    /// Apply overrides from CLI/environment variables
    ///
    /// This method modifies the config in-place, applying any overrides
    /// that are set (Some values). The precedence is:
    /// CLI args > Env vars > Config file > Defaults
    ///
    /// Note: clap handles CLI > Env precedence automatically when using
    /// the `env` attribute, so by the time we receive ConfigOverrides,
    /// the correct precedence is already applied.
    pub fn apply_overrides(&mut self, overrides: &ConfigOverrides) {
        if let Some(port) = overrides.port {
            tracing::debug!(port, "Applying port override");
            self.server.port = port;
        }

        if let Some(ref bind_address) = overrides.bind_address {
            tracing::debug!(bind_address, "Applying bind_address override");
            self.server.bind_address = bind_address.clone();
        }

        if let Some(ref metrics_path) = overrides.metrics_path {
            tracing::debug!(metrics_path, "Applying metrics_path override");
            self.server.path = metrics_path.clone();
        }

        if let Some(ref jolokia_url) = overrides.jolokia_url {
            tracing::debug!(jolokia_url, "Applying jolokia_url override");
            self.jolokia.url = jolokia_url.clone();
        }

        if let Some(timeout) = overrides.jolokia_timeout {
            tracing::debug!(timeout_ms = timeout, "Applying jolokia_timeout override");
            self.jolokia.timeout_ms = timeout;
        }

        if let Some(ref username) = overrides.username {
            tracing::debug!("Applying username override");
            self.jolokia.username = Some(username.clone());
        }

        if let Some(ref password) = overrides.password {
            tracing::debug!("Applying password override");
            self.jolokia.password = Some(password.clone());
        }

        if let Some(tls_enabled) = overrides.tls_enabled {
            tracing::debug!(tls_enabled, "Applying tls_enabled override");
            self.server.tls.enabled = tls_enabled;
        }

        if let Some(ref tls_cert_file) = overrides.tls_cert_file {
            tracing::debug!(tls_cert_file, "Applying tls_cert_file override");
            self.server.tls.cert_file = Some(tls_cert_file.clone());
        }

        if let Some(ref tls_key_file) = overrides.tls_key_file {
            tracing::debug!(tls_key_file, "Applying tls_key_file override");
            self.server.tls.key_file = Some(tls_key_file.clone());
        }
    }

    /// Validate the final configuration after all overrides are applied
    ///
    /// This performs validation that was skipped in the initial load
    /// because CLI/env overrides may change values.
    pub fn validate_final(&self) -> Result<(), ConfigError> {
        // Validate port
        Self::validate_port(self.server.port)?;

        // Validate metrics path (in case it was overridden)
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

        // Validate TLS configuration
        if self.server.tls.enabled {
            if self.server.tls.cert_file.is_none() {
                return Err(ConfigError::ValidationError(
                    "TLS is enabled but cert_file is not specified".to_string(),
                ));
            }
            if self.server.tls.key_file.is_none() {
                return Err(ConfigError::ValidationError(
                    "TLS is enabled but key_file is not specified".to_string(),
                ));
            }
        }

        Ok(())
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

        // Validate TLS configuration
        if self.server.tls.enabled {
            if self.server.tls.cert_file.is_none() {
                return Err(ConfigError::ValidationError(
                    "TLS is enabled but cert_file is not specified".to_string(),
                ));
            }
            if self.server.tls.key_file.is_none() {
                return Err(ConfigError::ValidationError(
                    "TLS is enabled but key_file is not specified".to_string(),
                ));
            }
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

    #[test]
    fn test_tls_config_default() {
        let config = TlsConfig::default();
        assert!(!config.enabled);
        assert!(config.cert_file.is_none());
        assert!(config.key_file.is_none());
    }

    #[test]
    fn test_tls_config_enabled_without_cert() {
        let yaml = r#"
server:
  tls:
    enabled: true
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_tls_config_enabled_without_key() {
        let yaml = r#"
server:
  tls:
    enabled: true
    cert_file: "/path/to/cert.pem"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.validate().is_err());
    }

    #[test]
    fn test_tls_config_valid() {
        let yaml = r#"
server:
  tls:
    enabled: true
    cert_file: "/path/to/cert.pem"
    key_file: "/path/to/key.pem"
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
        assert!(config.server.tls.enabled);
        assert_eq!(
            config.server.tls.cert_file,
            Some("/path/to/cert.pem".to_string())
        );
        assert_eq!(
            config.server.tls.key_file,
            Some("/path/to/key.pem".to_string())
        );
    }

    #[test]
    fn test_tls_config_disabled_no_files_required() {
        let yaml = r#"
server:
  tls:
    enabled: false
"#;
        let config: Config = serde_yaml::from_str(yaml).unwrap();
        assert!(config.validate().is_ok());
        assert!(!config.server.tls.enabled);
    }

    #[test]
    fn test_apply_tls_overrides() {
        let mut config = Config::default();
        assert!(!config.server.tls.enabled);
        assert!(config.server.tls.cert_file.is_none());
        assert!(config.server.tls.key_file.is_none());

        let overrides = ConfigOverrides {
            tls_enabled: Some(true),
            tls_cert_file: Some("/path/to/cert.pem".to_string()),
            tls_key_file: Some("/path/to/key.pem".to_string()),
            ..Default::default()
        };

        config.apply_overrides(&overrides);

        assert!(config.server.tls.enabled);
        assert_eq!(
            config.server.tls.cert_file,
            Some("/path/to/cert.pem".to_string())
        );
        assert_eq!(
            config.server.tls.key_file,
            Some("/path/to/key.pem".to_string())
        );
    }

    #[test]
    fn test_validate_final_with_tls() {
        let mut config = Config::default();
        config.server.tls.enabled = true;
        config.server.tls.cert_file = Some("/path/to/cert.pem".to_string());
        config.server.tls.key_file = Some("/path/to/key.pem".to_string());

        assert!(config.validate_final().is_ok());
    }

    #[test]
    fn test_validate_final_tls_missing_cert() {
        let mut config = Config::default();
        config.server.tls.enabled = true;
        config.server.tls.key_file = Some("/path/to/key.pem".to_string());

        assert!(config.validate_final().is_err());
    }

    #[test]
    fn test_validate_final_tls_missing_key() {
        let mut config = Config::default();
        config.server.tls.enabled = true;
        config.server.tls.cert_file = Some("/path/to/cert.pem".to_string());

        assert!(config.validate_final().is_err());
    }

    #[test]
    fn test_config_overrides_default() {
        let overrides = ConfigOverrides::default();
        assert!(overrides.port.is_none());
        assert!(overrides.bind_address.is_none());
        assert!(overrides.metrics_path.is_none());
        assert!(overrides.jolokia_url.is_none());
        assert!(overrides.jolokia_timeout.is_none());
        assert!(overrides.username.is_none());
        assert!(overrides.password.is_none());
        assert!(overrides.tls_enabled.is_none());
        assert!(overrides.tls_cert_file.is_none());
        assert!(overrides.tls_key_file.is_none());
    }

    #[test]
    fn test_apply_overrides_port() {
        let mut config = Config::default();
        assert_eq!(config.server.port, 9090);

        let overrides = ConfigOverrides {
            port: Some(8080),
            ..Default::default()
        };
        config.apply_overrides(&overrides);
        assert_eq!(config.server.port, 8080);
    }

    #[test]
    fn test_apply_overrides_bind_address() {
        let mut config = Config::default();
        assert_eq!(config.server.bind_address, "0.0.0.0");

        let overrides = ConfigOverrides {
            bind_address: Some("127.0.0.1".to_string()),
            ..Default::default()
        };
        config.apply_overrides(&overrides);
        assert_eq!(config.server.bind_address, "127.0.0.1");
    }

    #[test]
    fn test_apply_overrides_metrics_path() {
        let mut config = Config::default();
        assert_eq!(config.server.path, "/metrics");

        let overrides = ConfigOverrides {
            metrics_path: Some("/custom-metrics".to_string()),
            ..Default::default()
        };
        config.apply_overrides(&overrides);
        assert_eq!(config.server.path, "/custom-metrics");
    }

    #[test]
    fn test_apply_overrides_jolokia_url() {
        let mut config = Config::default();
        assert_eq!(config.jolokia.url, "http://localhost:8778/jolokia");

        let overrides = ConfigOverrides {
            jolokia_url: Some("http://example.com:9999/jolokia".to_string()),
            ..Default::default()
        };
        config.apply_overrides(&overrides);
        assert_eq!(config.jolokia.url, "http://example.com:9999/jolokia");
    }

    #[test]
    fn test_apply_overrides_jolokia_timeout() {
        let mut config = Config::default();
        assert_eq!(config.jolokia.timeout_ms, 5000);

        let overrides = ConfigOverrides {
            jolokia_timeout: Some(10000),
            ..Default::default()
        };
        config.apply_overrides(&overrides);
        assert_eq!(config.jolokia.timeout_ms, 10000);
    }

    #[test]
    fn test_apply_overrides_credentials() {
        let mut config = Config::default();
        assert!(config.jolokia.username.is_none());
        assert!(config.jolokia.password.is_none());

        let overrides = ConfigOverrides {
            username: Some("admin".to_string()),
            password: Some("secret".to_string()),
            ..Default::default()
        };
        config.apply_overrides(&overrides);
        assert_eq!(config.jolokia.username, Some("admin".to_string()));
        assert_eq!(config.jolokia.password, Some("secret".to_string()));
    }

    #[test]
    fn test_apply_overrides_all() {
        let mut config = Config::default();

        let overrides = ConfigOverrides {
            port: Some(8080),
            bind_address: Some("127.0.0.1".to_string()),
            metrics_path: Some("/custom-metrics".to_string()),
            jolokia_url: Some("http://example.com:9999/jolokia".to_string()),
            jolokia_timeout: Some(15000),
            username: Some("user".to_string()),
            password: Some("pass".to_string()),
            tls_enabled: Some(true),
            tls_cert_file: Some("/path/to/cert.pem".to_string()),
            tls_key_file: Some("/path/to/key.pem".to_string()),
        };
        config.apply_overrides(&overrides);

        assert_eq!(config.server.port, 8080);
        assert_eq!(config.server.bind_address, "127.0.0.1");
        assert_eq!(config.server.path, "/custom-metrics");
        assert_eq!(config.jolokia.url, "http://example.com:9999/jolokia");
        assert_eq!(config.jolokia.timeout_ms, 15000);
        assert_eq!(config.jolokia.username, Some("user".to_string()));
        assert_eq!(config.jolokia.password, Some("pass".to_string()));
        assert!(config.server.tls.enabled);
        assert_eq!(
            config.server.tls.cert_file,
            Some("/path/to/cert.pem".to_string())
        );
        assert_eq!(
            config.server.tls.key_file,
            Some("/path/to/key.pem".to_string())
        );
    }

    #[test]
    fn test_apply_overrides_none_preserves_config() {
        let mut config = Config::default();
        config.server.port = 8080;
        config.jolokia.url = "http://custom:8778/jolokia".to_string();

        let overrides = ConfigOverrides::default();
        config.apply_overrides(&overrides);

        // Should preserve original values when overrides are None
        assert_eq!(config.server.port, 8080);
        assert_eq!(config.jolokia.url, "http://custom:8778/jolokia");
    }

    #[test]
    fn test_validate_final_valid() {
        let config = Config::default();
        assert!(config.validate_final().is_ok());
    }

    #[test]
    fn test_validate_final_invalid_port() {
        let mut config = Config::default();
        config.server.port = 0;
        assert!(config.validate_final().is_err());
    }

    #[test]
    fn test_validate_final_invalid_metrics_path() {
        let mut config = Config::default();
        config.server.path = "no-slash".to_string();
        let err = config.validate_final();
        assert!(err.is_err());
        assert!(err
            .unwrap_err()
            .to_string()
            .contains("Metrics path must start with '/'"));
    }

    #[test]
    fn test_validate_final_conflicting_metrics_path() {
        let mut config = Config::default();
        config.server.path = "/health".to_string();
        let err = config.validate_final();
        assert!(err.is_err());
        assert!(err.unwrap_err().to_string().contains("must not conflict"));
    }
}
