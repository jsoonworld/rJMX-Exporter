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
    config::Config,
    server,
    transformer::convert_java_regex,
};

#[tokio::main]
async fn main() -> Result<()> {
    // Record startup time
    let start_time = Instant::now();

    // Parse CLI arguments
    let cli = Cli::parse();

    // Initialize logging
    rjmx_exporter::init_logging(&cli.log_level.to_string())?;

    // Load configuration
    let config = Config::load_or_default(&cli.config)?;

    // Handle --validate mode
    if cli.validate {
        return validate_config(&config, &cli);
    }

    // Handle --dry-run mode
    if cli.dry_run {
        return dry_run(&config, &cli);
    }

    // Apply CLI port override, then validate the final port value
    let port = cli.port.unwrap_or(config.server.port);
    Config::validate_port(port)?;

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

    // Start server
    server::run(config, port).await?;

    Ok(())
}

/// Validate configuration and display results
fn validate_config(config: &Config, cli: &Cli) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    // Validate port (apply CLI override if provided)
    let port = cli.port.unwrap_or(config.server.port);
    if let Err(e) = Config::validate_port(port) {
        errors.push(format!("Invalid port: {}", e));
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
                println!("  Server port: {}", port);
                println!("  Metrics path: {}", config.server.path);
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
                "server_port": port,
                "metrics_path": config.server.path,
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
                "server_port": port,
                "metrics_path": config.server.path,
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
fn dry_run(config: &Config, cli: &Cli) -> Result<()> {
    let mut errors: Vec<String> = Vec::new();

    // Validate port (apply CLI override if provided)
    let port = cli.port.unwrap_or(config.server.port);
    if let Err(e) = Config::validate_port(port) {
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
            println!("  Server port: {}", port);
            println!("  Metrics path: {}", config.server.path);

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
                "server_port": port,
                "metrics_path": config.server.path,
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
                "server_port": port,
                "metrics_path": config.server.path,
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
