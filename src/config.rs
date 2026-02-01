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

    /// Server bind address
    #[serde(default = "default_bind_address")]
    pub bind_address: String,
}

/// Metric transformation rule
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Rule {
    /// MBean pattern to match
    pub pattern: String,

    /// Prometheus metric name (supports $1, $2, etc.)
    pub name: String,

    /// Metric type (gauge, counter, untyped)
    #[serde(default = "default_metric_type")]
    pub r#type: String,

    /// Optional help text
    pub help: Option<String>,

    /// Optional static labels
    #[serde(default)]
    pub labels: std::collections::HashMap<String, String>,
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

        if !path.exists() {
            tracing::warn!(
                path = %path.display(),
                "Config file not found, using defaults"
            );
            return Ok(Self::default());
        }

        Self::load(path)
    }

    /// Validate the configuration
    fn validate(&self) -> Result<(), ConfigError> {
        if self.server.port == 0 {
            return Err(ConfigError::ValidationError(
                "Server port must be greater than 0".to_string(),
            ));
        }

        if !self.server.path.starts_with('/') {
            return Err(ConfigError::ValidationError(
                "Metrics path must start with '/'".to_string(),
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
    fn test_config_validation() {
        let mut config = Config::default();
        config.server.port = 0;
        assert!(config.validate().is_err());
    }
}
