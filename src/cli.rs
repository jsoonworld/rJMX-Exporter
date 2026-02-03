//! CLI argument parsing for rJMX-Exporter
//!
//! This module provides the command-line interface using clap derive macros.
//!
//! # Options
//!
//! - `--config` / `-c`: Configuration file path (default: config.yaml)
//! - `--port` / `-p`: Server port (overrides config file)
//! - `--validate`: Validate configuration without starting server
//! - `--dry-run`: Test configuration and show parsed rules
//! - `--log-level` / `-l`: Log level (trace/debug/info/warn/error)
//! - `--output-format`: Output format for validate/dry-run (text/json/yaml)
//! - `--startup-time`: Measure and display startup time

use clap::{Parser, ValueEnum};
use std::path::PathBuf;

/// rJMX-Exporter - High-performance JMX Metric Exporter written in Rust
///
/// Collects JMX metrics from Java applications via Jolokia
/// and exports them in Prometheus format.
#[derive(Parser, Debug)]
#[command(name = "rjmx-exporter")]
#[command(author, version, about, long_about = None)]
#[command(propagate_version = true)]
pub struct Cli {
    /// Path to configuration file
    #[arg(short, long, value_name = "FILE", default_value = "config.yaml")]
    pub config: PathBuf,

    /// Server port (overrides config file)
    #[arg(short, long, value_name = "PORT", env = "RJMX_PORT")]
    pub port: Option<u16>,

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
}
