//! rJMX-Exporter - High-performance JMX metrics exporter
//!
//! This binary provides a Prometheus-compatible metrics endpoint
//! that collects JMX metrics from Java applications via Jolokia.

use std::time::Instant;

use anyhow::Result;
use clap::Parser;
use tracing::info;

use rjmx_exporter::{
    cli::{Cli, OutputFormat},
    config::{Config, ConfigOverrides},
    server,
    transformer::convert_java_regex,
};

/// Create ConfigOverrides from CLI arguments
///
/// CLI arguments include values from environment variables (handled by clap),
/// so this gives us the correct precedence: CLI > Env > Config file > Defaults
fn cli_to_overrides(cli: &Cli) -> ConfigOverrides {
    ConfigOverrides {
        port: cli.port,
        bind_address: cli.bind_address.clone(),
        metrics_path: cli.metrics_path.clone(),
        jolokia_url: cli.jolokia_url.clone(),
        jolokia_timeout: cli.jolokia_timeout,
        username: cli.username.clone(),
        password: cli.password.clone(),
        tls_enabled: cli.tls_enabled,
        tls_cert_file: cli.tls_cert_file.clone(),
        tls_key_file: cli.tls_key_file.clone(),
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Record startup time
    let start_time = Instant::now();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    rjmx_exporter::init_logging(&cli.log_level.to_string())?;

    // Load configuration from file
    let mut config = Config::load_or_default(&cli.config)?;

    // Apply CLI/env overrides (precedence: CLI > Env > Config file > Defaults)
    let overrides = cli_to_overrides(&cli);
    config.apply_overrides(&overrides);

    // Handle --validate mode
    if cli.validate {
        return validate_config(&config, &cli);
    }

    // Handle --dry-run mode
    if cli.dry_run {
        return dry_run(&config, &cli);
    }

    // Validate final configuration after all overrides are applied
    config.validate_final()?;

    // Calculate startup duration
    let startup_duration = start_time.elapsed();

    // Log startup info
    if cli.startup_time {
        info!(
            startup_ms = startup_duration.as_millis(),
            version = env!("CARGO_PKG_VERSION"),
            "Startup completed"
        );
        println!("Startup time: {}ms", startup_duration.as_millis());
    } else {
        info!(
            version = env!("CARGO_PKG_VERSION"),
            startup_ms = startup_duration.as_millis(),
            "Starting rJMX-Exporter"
        );
    }

    // Start server (port is now part of config)
    server::run(config).await?;

    Ok(())
}

/// Validate configuration and display results
///
/// Note: Config already has CLI/env overrides applied at this point
fn validate_config(config: &Config, cli: &Cli) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    // Validate port (overrides already applied to config)
    if let Err(e) = Config::validate_port(config.server.port) {
        errors.push(format!("Invalid port: {}", e));
    }

    // Validate metrics path
    if !config.server.path.starts_with('/') {
        errors.push("Metrics path must start with '/'".to_string());
    } else if config.server.path == "/" || config.server.path == "/health" {
        errors.push("Metrics path must not conflict with '/' or '/health'".to_string());
    }

    // Validate TLS configuration
    if config.server.tls.enabled {
        if config.server.tls.cert_file.is_none() {
            errors.push("TLS is enabled but cert_file is not specified".to_string());
        }
        if config.server.tls.key_file.is_none() {
            errors.push("TLS is enabled but key_file is not specified".to_string());
        }
    }

    // Validate rule patterns (convert Java regex to Rust regex)
    for (i, rule) in config.rules.iter().enumerate() {
        match convert_java_regex(&rule.pattern) {
            Ok(converted_pattern) => {
                if let Err(e) = regex::Regex::new(&converted_pattern) {
                    errors.push(format!(
                        "Rule {}: Invalid regex after conversion: {} (original: {}, converted: {})",
                        i, e, rule.pattern, converted_pattern
                    ));
                }
            }
            Err(e) => {
                errors.push(format!("Rule {}: Regex conversion error: {}", i, e));
            }
        }
    }

    let is_valid = errors.is_empty();

    match cli.output_format {
        OutputFormat::Text => {
            if is_valid {
                println!("Configuration is valid");
                println!("  Config file: {}", cli.config.display());
                println!("  Jolokia URL: {}", config.jolokia.url);
                println!("  Server port: {}", config.server.port);
                println!("  Bind address: {}", config.server.bind_address);
                println!("  Metrics path: {}", config.server.path);
                println!("  TLS enabled: {}", config.server.tls.enabled);
                println!("  Rules: {}", config.rules.len());
            } else {
                eprintln!("Configuration validation failed:");
                for error in &errors {
                    eprintln!("  - {}", error);
                }
            }
        }
        OutputFormat::Json => {
            let result = serde_json::json!({
                "valid": is_valid,
                "config_file": cli.config.display().to_string(),
                "jolokia_url": config.jolokia.url,
                "server_port": config.server.port,
                "bind_address": config.server.bind_address,
                "metrics_path": config.server.path,
                "tls_enabled": config.server.tls.enabled,
                "rules_count": config.rules.len(),
                "errors": errors
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "valid": is_valid,
                "config_file": cli.config.display().to_string(),
                "jolokia_url": config.jolokia.url,
                "server_port": config.server.port,
                "bind_address": config.server.bind_address,
                "metrics_path": config.server.path,
                "tls_enabled": config.server.tls.enabled,
                "rules_count": config.rules.len(),
                "errors": errors
            });
            println!("{}", serde_yaml::to_string(&result)?);
        }
    }

    if is_valid {
        Ok(())
    } else {
        anyhow::bail!(
            "Configuration validation failed with {} error(s)",
            errors.len()
        )
    }
}

/// Dry run: test configuration and show parsed rules
///
/// Note: Config already has CLI/env overrides applied at this point
fn dry_run(config: &Config, cli: &Cli) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    // Validate port (overrides already applied to config)
    if let Err(e) = Config::validate_port(config.server.port) {
        errors.push(format!("Invalid port: {}", e));
    }

    // Compile all rules to verify they work
    let mut compiled_rules: Vec<serde_json::Value> = Vec::new();

    for (i, rule) in config.rules.iter().enumerate() {
        let conversion_result = convert_java_regex(&rule.pattern);
        let (converted_pattern, conversion_error) = match conversion_result {
            Ok(p) => (p, None),
            Err(e) => (rule.pattern.clone(), Some(e.to_string())),
        };
        let regex_result = regex::Regex::new(&converted_pattern);

        let is_valid = conversion_error.is_none() && regex_result.is_ok();
        if !is_valid {
            errors.push(format!(
                "Rule {} is invalid (pattern: {})",
                i + 1,
                rule.pattern
            ));
        }

        let rule_info = serde_json::json!({
            "index": i + 1,
            "pattern": rule.pattern,
            "converted_pattern": converted_pattern,
            "name": rule.name,
            "type": rule.r#type,
            "help": rule.help,
            "labels": rule.labels,
            "valid": is_valid,
            "conversion_error": conversion_error,
            "regex_error": regex_result.as_ref().err().map(|e| e.to_string())
        });

        compiled_rules.push(rule_info);
    }

    let valid_count = compiled_rules
        .iter()
        .filter(|r| r["valid"].as_bool().unwrap_or(false))
        .count();

    match cli.output_format {
        OutputFormat::Text => {
            println!("Dry run completed");
            println!(
                "Loaded {} rule(s) ({} valid)",
                config.rules.len(),
                valid_count
            );
            println!();
            println!("Configuration:");
            println!("  Config file: {}", cli.config.display());
            println!("  Jolokia URL: {}", config.jolokia.url);
            println!("  Server port: {}", config.server.port);
            println!("  Bind address: {}", config.server.bind_address);
            println!("  Metrics path: {}", config.server.path);
            println!("  TLS enabled: {}", config.server.tls.enabled);

            if !errors.is_empty() {
                println!();
                println!("Errors:");
                for error in &errors {
                    println!("  - {}", error);
                }
            }
            println!();

            for rule_info in &compiled_rules {
                let idx = rule_info["index"].as_u64().unwrap_or(0);
                let valid = rule_info["valid"].as_bool().unwrap_or(false);
                let status = if valid { "OK" } else { "INVALID" };

                println!("Rule {} [{}]:", idx, status);
                println!("  Pattern: {}", rule_info["pattern"].as_str().unwrap_or(""));
                println!(
                    "  Converted: {}",
                    rule_info["converted_pattern"].as_str().unwrap_or("")
                );
                println!("  Name: {}", rule_info["name"].as_str().unwrap_or(""));
                println!(
                    "  Type: {}",
                    rule_info["type"].as_str().unwrap_or("untyped")
                );

                if let Some(help) = rule_info["help"].as_str() {
                    println!("  Help: {}", help);
                }

                if let Some(error) = rule_info["conversion_error"].as_str() {
                    println!("  Conversion Error: {}", error);
                }

                if let Some(error) = rule_info["regex_error"].as_str() {
                    println!("  Regex Error: {}", error);
                }

                println!();
            }
        }
        OutputFormat::Json => {
            let result = serde_json::json!({
                "status": "dry_run_completed",
                "config_file": cli.config.display().to_string(),
                "jolokia_url": config.jolokia.url,
                "server_port": config.server.port,
                "bind_address": config.server.bind_address,
                "metrics_path": config.server.path,
                "tls_enabled": config.server.tls.enabled,
                "rules_count": config.rules.len(),
                "valid_rules_count": valid_count,
                "rules": compiled_rules,
                "errors": errors
            });
            println!("{}", serde_json::to_string_pretty(&result)?);
        }
        OutputFormat::Yaml => {
            let result = serde_json::json!({
                "status": "dry_run_completed",
                "config_file": cli.config.display().to_string(),
                "jolokia_url": config.jolokia.url,
                "server_port": config.server.port,
                "bind_address": config.server.bind_address,
                "metrics_path": config.server.path,
                "tls_enabled": config.server.tls.enabled,
                "rules_count": config.rules.len(),
                "valid_rules_count": valid_count,
                "rules": compiled_rules,
                "errors": errors
            });
            println!("{}", serde_yaml::to_string(&result)?);
        }
    }

    if errors.is_empty() {
        Ok(())
    } else {
        anyhow::bail!("Dry run failed with {} error(s)", errors.len())
    }
}
