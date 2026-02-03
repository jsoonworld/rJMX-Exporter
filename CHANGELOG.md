# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

### Added

- **HTTPS/TLS Support** (Issue #32)
  - Optional TLS/HTTPS for the `/metrics` endpoint
  - Configuration via YAML (`server.tls.enabled`, `server.tls.cert_file`, `server.tls.key_file`)
  - Environment variables: `RJMX_TLS_ENABLED`, `RJMX_TLS_CERT_FILE`, `RJMX_TLS_KEY_FILE`
  - CLI arguments: `--tls-enabled`, `--tls-cert-file`, `--tls-key-file`
  - PEM format certificate support via axum-server with rustls

### Changed

### Deprecated

### Removed

### Fixed

### Security

---

## [0.1.0] - 2026-02-03

### Added

- **Core Functionality**
  - Native Rust binary with no JVM dependency
  - Prometheus `/metrics` endpoint with text exposition format
  - Rule-based MBean to metric transformation
  - jmx_exporter YAML configuration compatibility

- **Collector Module**
  - Jolokia HTTP client with connection pooling
  - Basic authentication support
  - Configurable timeouts and retries
  - Bulk MBean operations
  - MBean search functionality

- **Transformation Engine**
  - Regex pattern matching with capture groups
  - Dynamic label extraction (`$1`, `$2` substitution)
  - Whitelist/blacklist MBean filtering
  - Metric name and label validation
  - Java regex to Rust regex conversion

- **Server**
  - Tokio + Axum based HTTP server
  - Health check endpoint (`/health`)
  - Graceful shutdown support

- **Operations**
  - CLI with validation and dry-run modes
  - YAML configuration support
  - Docker and Docker Compose support
  - Comprehensive test suite (108+ tests)

- **Documentation**
  - Complete README with Quick Start guide
  - Migration guide from jmx_exporter
  - CLI reference and configuration examples
  - Contributing guidelines and code of conduct

[Unreleased]: https://github.com/jsoonworld/rJMX-Exporter/compare/v0.1.0...HEAD
[0.1.0]: https://github.com/jsoonworld/rJMX-Exporter/releases/tag/v0.1.0
