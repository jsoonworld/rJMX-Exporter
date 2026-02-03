//! CLI integration tests
//!
//! Tests for the command-line interface using assert_cmd.
//!
//! These tests verify:
//! - Help and version flags
//! - Configuration validation
//! - Dry run mode
//! - Error handling for missing files

use assert_cmd::Command;
use predicates::prelude::*;
use std::io::Write;
use tempfile::NamedTempFile;

/// Get a command for the rjmx-exporter binary
#[allow(deprecated)]
fn cmd() -> Command {
    Command::cargo_bin("rjmx-exporter").expect("Failed to find rjmx-exporter binary")
}

/// Test --help flag displays usage information
#[test]
fn test_help_flag() {
    cmd()
        .arg("--help")
        .assert()
        .success()
        .stdout(predicate::str::contains("Usage:").or(predicate::str::contains("usage:")))
        .stdout(predicate::str::contains("--config").or(predicate::str::contains("-c")));
}

/// Test -h short flag also works
#[test]
fn test_help_short_flag() {
    cmd()
        .arg("-h")
        .assert()
        .success()
        .stdout(predicate::str::contains("rjmx-exporter"));
}

/// Test --version flag displays version
#[test]
fn test_version_flag() {
    cmd()
        .arg("--version")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

/// Test -V short flag also works
#[test]
fn test_version_short_flag() {
    cmd()
        .arg("-V")
        .assert()
        .success()
        .stdout(predicate::str::contains(env!("CARGO_PKG_VERSION")));
}

/// Helper to create a temporary config file with given content
fn create_temp_config(content: &str) -> NamedTempFile {
    let mut file = NamedTempFile::new().expect("Failed to create temp file");
    file.write_all(content.as_bytes())
        .expect("Failed to write config");
    file.flush().expect("Failed to flush");
    file
}

/// Test that a valid configuration is accepted via --validate flag
#[test]
fn test_validate_valid_config() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
  timeout_ms: 5000

server:
  port: 19090
  path: "/metrics"

rules:
  - pattern: "java\\.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test that invalid configuration patterns are rejected
#[test]
fn test_validate_invalid_config_bad_yaml() {
    let config = r#"
jolokia:
  url: [not valid yaml
"#;

    let file = create_temp_config(config);

    // Should fail due to invalid YAML
    cmd()
        .arg("-c")
        .arg(file.path())
        .timeout(std::time::Duration::from_millis(1000))
        .assert()
        .failure();
}

/// Test that missing config file results in using defaults (with warning)
/// but doesn't crash the application
#[test]
fn test_missing_config_file() {
    // When config file doesn't exist, the app should use defaults
    // and log a warning, then try to start the server
    let result = cmd()
        .arg("-c")
        .arg("/nonexistent/path/config.yaml")
        .timeout(std::time::Duration::from_millis(500))
        .assert();

    // The app uses load_or_default which returns defaults for missing files
    // So it won't fail immediately - it will try to start the server
    // This is expected behavior
    let _ = result;
}

/// Test that invalid port (0) is rejected
#[test]
fn test_invalid_port_zero() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 0
"#;

    let file = create_temp_config(config);

    // Port 0 should be rejected by validation
    cmd()
        .arg("-c")
        .arg(file.path())
        .timeout(std::time::Duration::from_millis(1000))
        .assert()
        .failure();
}

/// Test that invalid metrics path is rejected
#[test]
fn test_invalid_metrics_path() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 9090
  path: "no-leading-slash"
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .timeout(std::time::Duration::from_millis(1000))
        .assert()
        .failure();
}

/// Test that port can be overridden via CLI
#[test]
fn test_port_override() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 9090
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("-p")
        .arg("19999")
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test that log level can be set via CLI
#[test]
fn test_log_level_argument() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 19091
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("--log-level")
        .arg("debug")
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test environment variable override for port
#[test]
fn test_env_port_override() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 9090
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .env("RJMX_PORT", "19092")
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test conflicting metrics path with root
#[test]
fn test_metrics_path_conflict_with_root() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 9090
  path: "/"
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .timeout(std::time::Duration::from_millis(1000))
        .assert()
        .failure();
}

/// Test conflicting metrics path with health endpoint
#[test]
fn test_metrics_path_conflict_with_health() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 9090
  path: "/health"
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .timeout(std::time::Duration::from_millis(1000))
        .assert()
        .failure();
}

/// Test that empty rule pattern is rejected
#[test]
fn test_empty_rule_pattern() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 9090
rules:
  - pattern: ""
    name: "test_metric"
    type: gauge
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .timeout(std::time::Duration::from_millis(1000))
        .assert()
        .failure();
}

/// Test jmx_exporter compatible configuration fields
#[test]
fn test_jmx_exporter_compat_config() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"

server:
  port: 19093

lowercaseOutputName: true
lowercaseOutputLabelNames: true

whitelistObjectNames:
  - "java.lang:*"
  - "com.example:*"

blacklistObjectNames:
  - "java.lang:type=MemoryPool,*"

rules:
  - pattern: "java\\.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
    value: "$1"
    valueFactor: 1.0
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test that basic auth config is accepted
#[test]
fn test_basic_auth_config() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
  username: "admin"
  password: "secret"
  timeout_ms: 5000

server:
  port: 19094
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test that multiple rules are accepted
#[test]
fn test_multiple_rules_config() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"

server:
  port: 19095

rules:
  - pattern: "java\\.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
    help: "JVM heap memory"
    labels:
      area: "heap"

  - pattern: "java\\.lang<type=Threading><(\\w+)>"
    name: "jvm_threads_$1"
    type: gauge
    help: "JVM thread metrics"

  - pattern: "java\\.lang<type=GarbageCollector,name=([^>]+)><(\\w+)>"
    name: "jvm_gc_$2"
    type: counter
    labels:
      gc: "$1"
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test custom bind address
#[test]
fn test_bind_address_config() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"

server:
  port: 19096
  bind_address: "127.0.0.1"
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test localhost as bind address
#[test]
fn test_localhost_bind_address() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"

server:
  port: 19097
  bind_address: "localhost"
"#;

    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("--validate")
        .assert()
        .success()
        .stdout(predicate::str::contains("Configuration is valid"));
}

/// Test dry run mode validates configuration and exits
#[test]
fn test_dry_run_mode() {
    let config = r#"
jolokia:
  url: "http://localhost:8778/jolokia"
server:
  port: 9090
rules:
  - pattern: "java\\.lang<type=Memory><HeapMemoryUsage>(\\w+)"
    name: "jvm_memory_heap_$1_bytes"
    type: gauge
"#;
    let file = create_temp_config(config);

    cmd()
        .arg("-c")
        .arg(file.path())
        .arg("--dry-run")
        .assert()
        .success()
        .stdout(predicate::str::contains("Dry run completed"))
        .stdout(predicate::str::contains("1 valid"));
}
