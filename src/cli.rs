//! CLI argument parsing for rJMX-Exporter
//!
//! This module provides the command-line interface using clap derive macros.
//!
//! # Options
//!
//! - `--config` / `-c`: Configuration file path (default: config.yaml, env: RJMX_CONFIG)
//! - `--port` / `-p`: Server port (overrides config file, env: RJMX_PORT)
//! - `--bind-address`: Server bind address (env: RJMX_BIND_ADDRESS)
//! - `--metrics-path`: Metrics endpoint path (env: RJMX_METRICS_PATH)
//! - `--jolokia-url`: Default Jolokia target URL (env: RJMX_JOLOKIA_URL)
//! - `--jolokia-timeout`: HTTP timeout in milliseconds (env: RJMX_JOLOKIA_TIMEOUT)
//! - `--username`: Jolokia auth username (env: RJMX_USERNAME)
//! - `--password`: Jolokia auth password (env: RJMX_PASSWORD)
//! - `--tls-enabled`: Enable TLS/HTTPS for the metrics endpoint (env: RJMX_TLS_ENABLED)
//! - `--tls-cert-file`: Path to TLS certificate file (env: RJMX_TLS_CERT_FILE)
//! - `--tls-key-file`: Path to TLS private key file (env: RJMX_TLS_KEY_FILE)
//! - `--validate`: Validate configuration without starting server
//! - `--dry-run`: Test configuration and show parsed rules
//! - `--log-level` / `-l`: Log level (trace/debug/info/warn/error, env: RJMX_LOG_LEVEL)
//! - `--output-format`: Output format for validate/dry-run (text/json/yaml)
//! - `--startup-time`: Measure and display startup time
//!
//! # Precedence
//!
//! Configuration values are resolved in the following order (highest to lowest priority):
//! 1. CLI arguments
//! 2. Environment variables
//! 3. Configuration file
//! 4. Default values

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// rJMX-Exporter - High-performance JMX Metric Exporter written in Rust
///
/// Collects JMX metrics from Java applications via Jolokia
/// and exports them in Prometheus format.
///
/// Environment variables can be used for all configuration options.
/// CLI arguments take precedence over environment variables,
/// which take precedence over config file values.
#[derive(Parser, Debug)]
#[command(name = "rjmx-exporter")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to configuration file
    #[arg(
        short,
        long,
        value_name = "FILE",
        default_value = "config.yaml",
        env = "RJMX_CONFIG"
    )]
    pub config: PathBuf,

    /// Server port (overrides config file)
    #[arg(short, long, value_name = "PORT", env = "RJMX_PORT")]
    pub port: Option<u16>,

    /// Server bind address (overrides config file)
    /// Supported values: IP addresses (0.0.0.0, 127.0.0.1, ::1) or "localhost"
    #[arg(long, value_name = "ADDRESS", env = "RJMX_BIND_ADDRESS")]
    pub bind_address: Option<String>,

    /// Metrics endpoint path (overrides config file)
    /// Must start with '/' and not conflict with '/' or '/health'
    #[arg(long, value_name = "PATH", env = "RJMX_METRICS_PATH")]
    pub metrics_path: Option<String>,

    /// Jolokia target URL (overrides config file)
    #[arg(long, value_name = "URL", env = "RJMX_JOLOKIA_URL")]
    pub jolokia_url: Option<String>,

    /// Jolokia HTTP timeout in milliseconds (overrides config file)
    #[arg(long, value_name = "MS", env = "RJMX_JOLOKIA_TIMEOUT")]
    pub jolokia_timeout: Option<u64>,

    /// Jolokia authentication username (overrides config file)
    #[arg(long, value_name = "USERNAME", env = "RJMX_USERNAME")]
    pub username: Option<String>,

    /// Jolokia authentication password (overrides config file)
    #[arg(long, value_name = "PASSWORD", env = "RJMX_PASSWORD")]
    pub password: Option<String>,

    /// Enable TLS/HTTPS for the metrics endpoint (overrides config file)
    #[arg(long, env = "RJMX_TLS_ENABLED")]
    pub tls_enabled: Option<bool>,

    /// Path to TLS certificate file in PEM format (overrides config file)
    #[arg(long, value_name = "FILE", env = "RJMX_TLS_CERT_FILE")]
    pub tls_cert_file: Option<String>,

    /// Path to TLS private key file in PEM format (overrides config file)
    #[arg(long, value_name = "FILE", env = "RJMX_TLS_KEY_FILE")]
    pub tls_key_file: Option<String>,

    /// Validate configuration without starting server
    #[arg(long)]
    pub validate: bool,

    /// Test configuration and show parsed rules
    #[arg(long)]
    pub dry_run: bool,

    /// Log level
    #[arg(
        short,
        long,
        value_enum,
        default_value = "info",
        env = "RJMX_LOG_LEVEL"
    )]
    pub log_level: LogLevel,

    /// Output format for --validate and --dry-run
    #[arg(long, value_enum, default_value = "text")]
    pub output_format: OutputFormat,

    /// Measure and display startup time
    #[arg(long)]
    pub startup_time: bool,
}

/// Log level options
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum LogLevel {
    /// Trace level - most verbose
    Trace,
    /// Debug level
    Debug,
    /// Info level - default
    Info,
    /// Warn level
    Warn,
    /// Error level - least verbose
    Error,
}

impl std::fmt::Display for LogLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LogLevel::Trace => write!(f, "trace"),
            LogLevel::Debug => write!(f, "debug"),
            LogLevel::Info => write!(f, "info"),
            LogLevel::Warn => write!(f, "warn"),
            LogLevel::Error => write!(f, "error"),
        }
    }
}

impl From<LogLevel> for tracing::Level {
    fn from(level: LogLevel) -> Self {
        match level {
            LogLevel::Trace => tracing::Level::TRACE,
            LogLevel::Debug => tracing::Level::DEBUG,
            LogLevel::Info => tracing::Level::INFO,
            LogLevel::Warn => tracing::Level::WARN,
            LogLevel::Error => tracing::Level::ERROR,
        }
    }
}

/// Output format options for validate and dry-run modes
#[derive(Copy, Clone, Debug, PartialEq, Eq, ValueEnum)]
pub enum OutputFormat {
    /// Human-readable text output
    Text,
    /// JSON output
    Json,
    /// YAML output
    Yaml,
}

impl std::fmt::Display for OutputFormat {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            OutputFormat::Text => write!(f, "text"),
            OutputFormat::Json => write!(f, "json"),
            OutputFormat::Yaml => write!(f, "yaml"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_log_level_display() {
        assert_eq!(LogLevel::Trace.to_string(), "trace");
        assert_eq!(LogLevel::Debug.to_string(), "debug");
        assert_eq!(LogLevel::Info.to_string(), "info");
        assert_eq!(LogLevel::Warn.to_string(), "warn");
        assert_eq!(LogLevel::Error.to_string(), "error");
    }

    #[test]
    fn test_log_level_conversion() {
        assert_eq!(tracing::Level::from(LogLevel::Trace), tracing::Level::TRACE);
        assert_eq!(tracing::Level::from(LogLevel::Debug), tracing::Level::DEBUG);
        assert_eq!(tracing::Level::from(LogLevel::Info), tracing::Level::INFO);
        assert_eq!(tracing::Level::from(LogLevel::Warn), tracing::Level::WARN);
        assert_eq!(tracing::Level::from(LogLevel::Error), tracing::Level::ERROR);
    }

    #[test]
    fn test_output_format_display() {
        assert_eq!(OutputFormat::Text.to_string(), "text");
        assert_eq!(OutputFormat::Json.to_string(), "json");
        assert_eq!(OutputFormat::Yaml.to_string(), "yaml");
    }

    #[test]
    fn test_cli_default_values() {
        let cli = Cli::parse_from(["rjmx-exporter"]);
        assert_eq!(cli.config, PathBuf::from("config.yaml"));
        assert_eq!(cli.port, None);
        assert_eq!(cli.bind_address, None);
        assert_eq!(cli.metrics_path, None);
        assert_eq!(cli.jolokia_url, None);
        assert_eq!(cli.jolokia_timeout, None);
        assert_eq!(cli.username, None);
        assert_eq!(cli.password, None);
        assert_eq!(cli.tls_enabled, None);
        assert_eq!(cli.tls_cert_file, None);
        assert_eq!(cli.tls_key_file, None);
        assert!(!cli.validate);
        assert!(!cli.dry_run);
        assert_eq!(cli.log_level, LogLevel::Info);
        assert_eq!(cli.output_format, OutputFormat::Text);
        assert!(!cli.startup_time);
    }

    #[test]
    fn test_cli_with_options() {
        let cli = Cli::parse_from([
            "rjmx-exporter",
            "-c",
            "custom.yaml",
            "-p",
            "8080",
            "--log-level",
            "debug",
            "--validate",
        ]);
        assert_eq!(cli.config, PathBuf::from("custom.yaml"));
        assert_eq!(cli.port, Some(8080));
        assert_eq!(cli.log_level, LogLevel::Debug);
        assert!(cli.validate);
    }

    #[test]
    fn test_cli_dry_run() {
        let cli = Cli::parse_from(["rjmx-exporter", "--dry-run", "--output-format", "json"]);
        assert!(cli.dry_run);
        assert_eq!(cli.output_format, OutputFormat::Json);
    }

    #[test]
    fn test_cli_startup_time() {
        let cli = Cli::parse_from(["rjmx-exporter", "--startup-time"]);
        assert!(cli.startup_time);
    }

    #[test]
    fn test_cli_new_options() {
        let cli = Cli::parse_from([
            "rjmx-exporter",
            "--bind-address",
            "127.0.0.1",
            "--metrics-path",
            "/custom-metrics",
            "--jolokia-url",
            "http://localhost:9999/jolokia",
            "--jolokia-timeout",
            "10000",
            "--username",
            "admin",
            "--password",
            "secret",
        ]);
        assert_eq!(cli.bind_address, Some("127.0.0.1".to_string()));
        assert_eq!(cli.metrics_path, Some("/custom-metrics".to_string()));
        assert_eq!(
            cli.jolokia_url,
            Some("http://localhost:9999/jolokia".to_string())
        );
        assert_eq!(cli.jolokia_timeout, Some(10000));
        assert_eq!(cli.username, Some("admin".to_string()));
        assert_eq!(cli.password, Some("secret".to_string()));
    }

    #[test]
    fn test_cli_tls_options() {
        let cli = Cli::parse_from([
            "rjmx-exporter",
            "--tls-enabled",
            "true",
            "--tls-cert-file",
            "/path/to/cert.pem",
            "--tls-key-file",
            "/path/to/key.pem",
        ]);
        assert_eq!(cli.tls_enabled, Some(true));
        assert_eq!(cli.tls_cert_file, Some("/path/to/cert.pem".to_string()));
        assert_eq!(cli.tls_key_file, Some("/path/to/key.pem".to_string()));
    }

    #[test]
    fn test_cli_tls_disabled() {
        let cli = Cli::parse_from(["rjmx-exporter", "--tls-enabled", "false"]);
        assert_eq!(cli.tls_enabled, Some(false));
        assert_eq!(cli.tls_cert_file, None);
        assert_eq!(cli.tls_key_file, None);
    }
}
