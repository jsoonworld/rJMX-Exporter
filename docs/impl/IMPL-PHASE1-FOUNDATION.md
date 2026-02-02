# Phase 1: Foundation - 구현 계획서

> **버전:** 1.0
> **작성일:** 2026-02-01
> **상태:** 계획 단계

---

## 1. 개요 (Overview)

Phase 1은 rJMX-Exporter 프로젝트의 기초를 다지는 단계입니다. 이 단계에서는 Rust 프로젝트 구조를 생성하고, 기본적인 HTTP 서버를 구현하며, 개발 및 테스트를 위한 Docker 환경을 구축합니다.

### 배경

rJMX-Exporter는 Java 기반의 jmx_exporter를 대체하는 고성능 Rust 기반 JMX 메트릭 수집기입니다. Jolokia를 통해 Java 애플리케이션의 JMX 메트릭을 수집하고 Prometheus 형식으로 내보냅니다.

### 아키텍처 개요

```text
┌─────────────────┐     ┌─────────────────┐     ┌─────────────────┐
│   Java App      │     │  rJMX-Exporter  │     │   Prometheus    │
│                 │     │     (Rust)      │     │                 │
│  ┌───────────┐  │     │                 │     │                 │
│  │  Jolokia  │◄─┼─────┤  Collector      │     │                 │
│  │  Agent    │  │JSON │       ↓         │     │                 │
│  └───────────┘  │     │  Transformer    │     │                 │
│                 │     │       ↓         │     │                 │
│                 │     │  /metrics  ◄────┼─────┤  Scraper        │
└─────────────────┘     └─────────────────┘     └─────────────────┘
```

---

## 2. 목표 (Goals)

### 주요 목표

| 목표 | 설명 | 성공 기준 |
|------|------|-----------|
| 프로젝트 구조 생성 | Cargo.toml 및 기본 모듈 구조 | `cargo build` 성공 |
| HTTP 서버 구현 | Tokio + Axum 기반 기본 서버 | 서버 시작 및 요청 응답 |
| 헬스체크 엔드포인트 | `/health` 엔드포인트 | HTTP 200 응답 |
| Docker 환경 | Java + Jolokia 테스트 환경 | `docker-compose up` 성공 |

### 성능 목표 (Phase 1 기준)

| 메트릭 | 목표값 | 비고 |
|--------|--------|------|
| 빌드 시간 | < 30초 | Debug 빌드 기준 |
| 바이너리 크기 | < 5MB | Release 빌드 기준 |
| 시작 시간 | < 50ms | 서버 준비까지 |
| 메모리 사용량 | < 5MB | 유휴 상태 기준 |

---

## 3. 구현 항목 (Implementation Items)

### 3.1 Rust 프로젝트 구조

- [ ] **Cargo.toml 생성**
  - 프로젝트 메타데이터 설정
  - 의존성 추가 (tokio, axum, serde, tracing 등)
  - 빌드 프로파일 최적화

- [ ] **기본 모듈 생성**
  - `src/main.rs` - 진입점
  - `src/lib.rs` - 라이브러리 루트
  - `src/config.rs` - 설정 관리
  - `src/error.rs` - 에러 타입 정의

- [ ] **서브 모듈 스켈레톤**
  - `src/collector/mod.rs` - 수집기 모듈 (스텁)
  - `src/transformer/mod.rs` - 변환기 모듈 (스텁)
  - `src/server/mod.rs` - HTTP 서버 모듈

### 3.2 Axum 서버 구현

- [ ] **기본 서버 설정**
  - Tokio 런타임 초기화
  - Axum 라우터 설정
  - Graceful shutdown 구현

- [ ] **엔드포인트 구현**
  - `GET /health` - 헬스체크
  - `GET /metrics` - 메트릭 (스텁, Phase 3에서 완성)
  - `GET /` - 루트 (정보 페이지)

- [ ] **로깅 설정**
  - tracing 초기화
  - 요청/응답 로깅 미들웨어

### 3.3 Docker 환경

- [ ] **docker-compose.yaml 작성**
  - Java 샘플 애플리케이션
  - Jolokia 에이전트 설정
  - 네트워크 구성

- [ ] **테스트 Java 애플리케이션**
  - 간단한 Spring Boot 앱 또는 JVM 프로세스
  - Jolokia JVM 에이전트 포함

---

## 4. 디렉토리 구조 (Directory Structure)

```
rJMX-Exporter/
├── Cargo.toml                    # 프로젝트 매니페스트
├── Cargo.lock                    # 의존성 잠금 파일
├── .gitignore                    # Git 무시 파일
├── CLAUDE.md                     # Claude 에이전트 컨텍스트
├── README.md                     # 프로젝트 설명
│
├── src/
│   ├── main.rs                   # 애플리케이션 진입점
│   ├── lib.rs                    # 라이브러리 루트
│   ├── config.rs                 # 설정 구조체 및 로딩
│   ├── error.rs                  # 에러 타입 정의
│   │
│   ├── collector/                # Jolokia 데이터 수집 (Phase 2)
│   │   └── mod.rs                # 모듈 스텁
│   │
│   ├── transformer/              # 메트릭 변환 (Phase 3)
│   │   └── mod.rs                # 모듈 스텁
│   │
│   └── server/                   # HTTP 서버
│       ├── mod.rs                # 서버 모듈 루트
│       └── handlers.rs           # 요청 핸들러
│
├── tests/
│   └── integration/              # 통합 테스트
│       └── server_test.rs        # 서버 테스트
│
├── examples/
│   └── docker-compose.yaml       # 개발 환경 Docker 구성
│
├── docker/
│   ├── java-app/                 # 테스트용 Java 앱
│   │   ├── Dockerfile
│   │   └── App.java
│   └── jolokia/                  # Jolokia 설정
│       └── jolokia.properties
│
└── docs/
    ├── 1-PAGER.md                # 프로젝트 설계 문서
    ├── TECH-STACK.md             # 기술 스택 문서
    └── IMPL-PHASE1-FOUNDATION.md # 이 문서
```

---

## 5. 상세 구현 계획 (Detailed Implementation Plan)

### 5.1 Cargo.toml 설정

```toml
[package]
name = "rjmx-exporter"
version = "0.1.0"
edition = "2021"
authors = ["rJMX-Exporter Team"]
description = "High-performance JMX metrics exporter written in Rust"
license = "MIT OR Apache-2.0"
repository = "https://github.com/your-org/rJMX-Exporter"
keywords = ["jmx", "prometheus", "metrics", "monitoring", "jolokia"]
categories = ["command-line-utilities", "development-tools"]

[dependencies]
# Async runtime
tokio = { version = "1.35", features = ["full"] }

# HTTP server
axum = { version = "0.7", features = ["macros"] }
tower = "0.4"
tower-http = { version = "0.5", features = ["trace", "cors"] }

# HTTP client (Phase 2에서 사용)
reqwest = { version = "0.11", default-features = false, features = ["json", "rustls-tls"] }

# Serialization
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
serde_yaml = "0.9"

# Logging
tracing = "0.1"
tracing-subscriber = { version = "0.3", features = ["env-filter", "json"] }

# Error handling
thiserror = "1.0"
anyhow = "1.0"

# CLI
clap = { version = "4.4", features = ["derive", "env"] }

# Utilities
once_cell = "1.19"

[dev-dependencies]
# Testing
tokio-test = "0.4"
reqwest = { version = "0.11", default-features = false, features = ["json", "rustls-tls"] }
assert_cmd = "2.0"
predicates = "3.0"

[profile.release]
lto = true
codegen-units = 1
panic = "abort"
strip = true

[profile.dev]
debug = true
```

### 5.2 기본 모듈 구조

#### 5.2.1 main.rs - 진입점

```rust
//! rJMX-Exporter - High-performance JMX metrics exporter
//!
//! This binary provides a Prometheus-compatible metrics endpoint
//! that collects JMX metrics from Java applications via Jolokia.

use anyhow::Result;
use clap::Parser;
use tracing::info;

use rjmx_exporter::{config::Config, server};

/// rJMX-Exporter CLI arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    /// Server port (overrides config file)
    #[arg(short, long, env = "RJMX_PORT")]
    port: Option<u16>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "RJMX_LOG_LEVEL")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize logging
    rjmx_exporter::init_logging(&args.log_level)?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting rJMX-Exporter"
    );

    // Load configuration
    let config = Config::load(&args.config)?;
    let port = args.port.unwrap_or(config.server.port);

    // Start server
    server::run(config, port).await?;

    Ok(())
}
```

#### 5.2.2 lib.rs - 라이브러리 루트

```rust
//! rJMX-Exporter library
//!
//! This crate provides the core functionality for collecting JMX metrics
//! from Java applications via Jolokia and exporting them in Prometheus format.

pub mod collector;
pub mod config;
pub mod error;
pub mod server;
pub mod transformer;

use anyhow::Result;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt, EnvFilter};

/// Initialize the logging subsystem
///
/// # Arguments
/// * `level` - Log level string (trace, debug, info, warn, error)
///
/// # Errors
/// Returns an error if the logging system fails to initialize
pub fn init_logging(level: &str) -> Result<()> {
    let filter = EnvFilter::try_from_default_env()
        .unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize logging: {}", e))?;

    Ok(())
}
```

#### 5.2.3 config.rs - 설정 관리

```rust
//! Configuration management for rJMX-Exporter
//!
//! Handles loading and validating configuration from YAML files.

use serde::{Deserialize, Serialize};
use std::path::Path;
use thiserror::Error;
use tracing;

/// Configuration errors
#[derive(Error, Debug)]
pub enum ConfigError {
    #[error("Failed to read config file: {0}")]
    ReadError(#[from] std::io::Error),

    #[error("Failed to parse config file: {0}")]
    ParseError(#[from] serde_yaml::Error),

    #[error("Invalid configuration: {0}")]
    ValidationError(String),
}

/// Main configuration structure
#[derive(Debug, Clone, Serialize, Deserialize)]
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

impl Default for Config {
    fn default() -> Self {
        Self {
            jolokia: JolokiaConfig::default(),
            server: ServerConfig::default(),
            rules: Vec::new(),
        }
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
```

#### 5.2.4 error.rs - 에러 타입 정의

```rust
//! Error types for rJMX-Exporter
//!
//! This module defines the error types used throughout the application.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

/// Application error type
#[derive(Error, Debug)]
pub enum AppError {
    /// Configuration error
    #[error("Configuration error: {0}")]
    Config(#[from] crate::config::ConfigError),

    /// HTTP client error
    #[error("HTTP client error: {0}")]
    HttpClient(String),

    /// Jolokia communication error
    #[error("Jolokia error: {0}")]
    Jolokia(String),

    /// Metric transformation error
    #[error("Transform error: {0}")]
    Transform(String),

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, message) = match self {
            AppError::Config(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()),
            AppError::HttpClient(e) => (StatusCode::BAD_GATEWAY, e),
            AppError::Jolokia(e) => (StatusCode::BAD_GATEWAY, e),
            AppError::Transform(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
            AppError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
        };

        tracing::error!(status = %status, error = %message, "Request failed");

        (status, message).into_response()
    }
}

/// Result type alias for application errors
pub type AppResult<T> = Result<T, AppError>;
```

### 5.3 Axum 서버 기본 구현

#### 5.3.1 server/mod.rs - 서버 모듈

```rust
//! HTTP server module
//!
//! Provides the Axum-based HTTP server for serving metrics.

pub mod handlers;

use std::net::SocketAddr;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::get, Router};
use tokio::signal;
use tower_http::trace::TraceLayer;
use tracing::info;

use crate::config::Config;

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    /// Application configuration
    pub config: Arc<Config>,
}

/// Run the HTTP server
///
/// # Arguments
/// * `config` - Application configuration
/// * `port` - Server port to bind to (overrides config.server.port)
///
/// # Errors
/// Returns an error if the server fails to start
pub async fn run(config: Config, port: u16) -> Result<()> {
    let bind_address = &config.server.bind_address;
    let metrics_path = &config.server.path;

    let state = AppState {
        config: Arc::new(config),
    };

    // Build router with configurable metrics path
    let app = Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health))
        .route(metrics_path, get(handlers::metrics))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Parse bind address from config
    let bind_addr: std::net::IpAddr = bind_address
        .parse()
        .unwrap_or_else(|_| std::net::IpAddr::V4(std::net::Ipv4Addr::new(0, 0, 0, 0)));
    let addr = SocketAddr::from((bind_addr, port));
    info!(address = %addr, metrics_path = %metrics_path, "Server listening");

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

/// Wait for shutdown signal
async fn shutdown_signal() {
    let ctrl_c = async {
        signal::ctrl_c()
            .await
            .expect("Failed to install Ctrl+C handler");
    };

    #[cfg(unix)]
    let terminate = async {
        signal::unix::signal(signal::unix::SignalKind::terminate())
            .expect("Failed to install signal handler")
            .recv()
            .await;
    };

    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();

    tokio::select! {
        _ = ctrl_c => {
            info!("Received Ctrl+C, shutting down");
        }
        _ = terminate => {
            info!("Received terminate signal, shutting down");
        }
    }
}
```

#### 5.3.2 server/handlers.rs - 요청 핸들러

```rust
//! HTTP request handlers
//!
//! Contains handlers for all HTTP endpoints.

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use serde::Serialize;

use super::AppState;

/// Health check response
#[derive(Serialize)]
pub struct HealthResponse {
    status: String,
    version: String,
}

/// Root endpoint - displays basic info
pub async fn root() -> Html<String> {
    let html = format!(
        r#"<!DOCTYPE html>
<html>
<head>
    <title>rJMX-Exporter</title>
</head>
<body>
    <h1>rJMX-Exporter</h1>
    <p>Version: {}</p>
    <ul>
        <li><a href="/health">Health Check</a></li>
        <li><a href="/metrics">Metrics</a></li>
    </ul>
</body>
</html>"#,
        env!("CARGO_PKG_VERSION")
    );
    Html(html)
}

/// Health check endpoint
pub async fn health() -> Json<HealthResponse> {
    Json(HealthResponse {
        status: "healthy".to_string(),
        version: env!("CARGO_PKG_VERSION").to_string(),
    })
}

/// Metrics endpoint (stub for Phase 1)
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    // Phase 1: Return stub metrics
    // Phase 2+: Collect from Jolokia and transform

    let stub_metrics = format!(
        r#"# HELP rjmx_exporter_info rJMX-Exporter information
# TYPE rjmx_exporter_info gauge
rjmx_exporter_info{{version="{}"}} 1
# HELP rjmx_exporter_scrape_duration_seconds Time spent scraping metrics
# TYPE rjmx_exporter_scrape_duration_seconds gauge
rjmx_exporter_scrape_duration_seconds 0.0
"#,
        env!("CARGO_PKG_VERSION")
    );

    (
        StatusCode::OK,
        [(axum::http::header::CONTENT_TYPE, "text/plain; charset=utf-8")],
        stub_metrics,
    )
}
```

### 5.4 스텁 모듈

#### 5.4.1 collector/mod.rs

```rust
//! JMX metrics collector module
//!
//! This module handles communication with Jolokia endpoints
//! and parsing of JMX metric data.
//!
//! **Status: Stub** - Implementation in Phase 2

// Placeholder for Phase 2 implementation
// pub mod client;
// pub mod parser;

/// Placeholder struct for the JMX collector
pub struct Collector;

impl Collector {
    /// Create a new collector (stub)
    pub fn new() -> Self {
        Self
    }
}

impl Default for Collector {
    fn default() -> Self {
        Self::new()
    }
}
```

#### 5.4.2 transformer/mod.rs

```rust
//! Metric transformation module
//!
//! This module handles transformation of JMX metrics
//! to Prometheus format based on configured rules.
//!
//! **Status: Stub** - Implementation in Phase 3

// Placeholder for Phase 3 implementation
// pub mod rules;

/// Placeholder struct for the metric transformer
pub struct Transformer;

impl Transformer {
    /// Create a new transformer (stub)
    pub fn new() -> Self {
        Self
    }
}

impl Default for Transformer {
    fn default() -> Self {
        Self::new()
    }
}
```

---

## 6. Docker 환경 설정 (Docker Environment)

### 6.1 docker-compose.yaml

```yaml
# Docker Compose configuration for rJMX-Exporter development
#
# Usage:
#   docker-compose up -d              # Start all services
#   docker-compose logs -f java-app   # View Java app logs
#   docker-compose down               # Stop all services

version: "3.8"

services:
  # Java application with Jolokia agent
  java-app:
    build:
      context: ./docker/java-app
      dockerfile: Dockerfile
    container_name: rjmx-java-app
    ports:
      - "8080:8080"   # Application port
      - "8778:8778"   # Jolokia port
    environment:
      - JAVA_OPTS=-javaagent:/opt/jolokia/jolokia-jvm.jar=port=8778,host=0.0.0.0
    healthcheck:
      test: ["CMD", "curl", "-f", "http://localhost:8778/jolokia/version"]
      interval: 10s
      timeout: 5s
      retries: 5
    networks:
      - rjmx-network

  # Prometheus for testing (optional)
  prometheus:
    image: prom/prometheus:latest
    container_name: rjmx-prometheus
    ports:
      - "9091:9090"
    volumes:
      - ./docker/prometheus/prometheus.yml:/etc/prometheus/prometheus.yml:ro
    command:
      - "--config.file=/etc/prometheus/prometheus.yml"
      - "--storage.tsdb.path=/prometheus"
      - "--web.enable-lifecycle"
    networks:
      - rjmx-network
    depends_on:
      - java-app

networks:
  rjmx-network:
    driver: bridge
```

### 6.2 Java 애플리케이션 Dockerfile

```dockerfile
# docker/java-app/Dockerfile
#
# Simple Java application with Jolokia agent for testing

FROM eclipse-temurin:17-jre-alpine

# Install curl for healthcheck
RUN apk add --no-cache curl

# Create app directory
WORKDIR /app

# Download Jolokia agent
ARG JOLOKIA_VERSION=1.7.2
RUN mkdir -p /opt/jolokia && \
    wget -q -O /opt/jolokia/jolokia-jvm.jar \
    "https://repo1.maven.org/maven2/org/jolokia/jolokia-jvm/${JOLOKIA_VERSION}/jolokia-jvm-${JOLOKIA_VERSION}.jar"

# Copy sample application
COPY App.java /app/

# Compile the sample app
RUN apk add --no-cache openjdk17 && \
    javac App.java && \
    apk del openjdk17

# Expose ports
EXPOSE 8080 8778

# Run with Jolokia agent
ENTRYPOINT ["sh", "-c", "java $JAVA_OPTS App"]
```

### 6.3 샘플 Java 애플리케이션

```java
// docker/java-app/App.java
//
// Simple Java application that exposes JMX metrics via Jolokia

import java.lang.management.ManagementFactory;
import java.lang.management.MemoryMXBean;
import java.lang.management.ThreadMXBean;
import java.util.Random;
import com.sun.net.httpserver.HttpServer;
import java.net.InetSocketAddress;

public class App {
    private static final Random random = new Random();
    private static volatile long requestCount = 0;

    public static void main(String[] args) throws Exception {
        System.out.println("Starting sample Java application...");
        System.out.println("Jolokia endpoint: http://localhost:8778/jolokia");

        // Start a simple HTTP server
        HttpServer server = HttpServer.create(new InetSocketAddress(8080), 0);

        server.createContext("/", exchange -> {
            requestCount++;
            String response = "Hello from Java! Request #" + requestCount;
            exchange.sendResponseHeaders(200, response.length());
            exchange.getResponseBody().write(response.getBytes());
            exchange.close();
        });

        server.createContext("/health", exchange -> {
            String response = "{\"status\":\"UP\"}";
            exchange.getResponseHeaders().set("Content-Type", "application/json");
            exchange.sendResponseHeaders(200, response.length());
            exchange.getResponseBody().write(response.getBytes());
            exchange.close();
        });

        server.setExecutor(null);
        server.start();

        System.out.println("HTTP server started on port 8080");

        // Generate some load for interesting metrics
        Thread loadGenerator = new Thread(() -> {
            while (true) {
                try {
                    // Allocate some memory occasionally
                    byte[] data = new byte[random.nextInt(1024 * 100)];
                    Thread.sleep(1000);
                } catch (InterruptedException e) {
                    break;
                }
            }
        });
        loadGenerator.setDaemon(true);
        loadGenerator.start();

        // Keep the application running
        Thread.currentThread().join();
    }
}
```

### 6.4 Prometheus 설정 (옵션)

```yaml
# docker/prometheus/prometheus.yml
#
# Prometheus configuration for testing rJMX-Exporter

global:
  scrape_interval: 15s
  evaluation_interval: 15s

scrape_configs:
  # Scrape rJMX-Exporter
  - job_name: "rjmx-exporter"
    static_configs:
      - targets: ["host.docker.internal:9090"]
    metrics_path: /metrics

  # Scrape Prometheus itself
  - job_name: "prometheus"
    static_configs:
      - targets: ["localhost:9090"]
```

---

## 7. 테스트 계획 (Test Plan)

### 7.1 단위 테스트

| 테스트 대상 | 테스트 항목 | 우선순위 |
|-------------|-------------|----------|
| `Config` | YAML 파싱 | P0 |
| `Config` | 기본값 적용 | P0 |
| `Config` | 유효성 검증 | P0 |
| `AppError` | 에러 변환 | P1 |

### 7.2 통합 테스트

```rust
// tests/integration/server_test.rs

use reqwest::Client;
use std::time::Duration;

#[tokio::test]
async fn test_health_endpoint() {
    // Start server in background
    let port = 19090; // Use different port for tests

    tokio::spawn(async move {
        let config = rjmx_exporter::config::Config::default();
        rjmx_exporter::server::run(config, port).await.unwrap();
    });

    // Wait for server to start
    tokio::time::sleep(Duration::from_millis(100)).await;

    // Test health endpoint
    let client = Client::new();
    let resp = client
        .get(format!("http://localhost:{}/health", port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body: serde_json::Value = resp.json().await.unwrap();
    assert_eq!(body["status"], "healthy");
}

#[tokio::test]
async fn test_metrics_endpoint() {
    let port = 19091;

    tokio::spawn(async move {
        let config = rjmx_exporter::config::Config::default();
        rjmx_exporter::server::run(config, port).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::new();
    let resp = client
        .get(format!("http://localhost:{}/metrics", port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert!(body.contains("rjmx_exporter_info"));
}

#[tokio::test]
async fn test_root_endpoint() {
    let port = 19092;

    tokio::spawn(async move {
        let config = rjmx_exporter::config::Config::default();
        rjmx_exporter::server::run(config, port).await.unwrap();
    });

    tokio::time::sleep(Duration::from_millis(100)).await;

    let client = Client::new();
    let resp = client
        .get(format!("http://localhost:{}/", port))
        .send()
        .await
        .expect("Failed to send request");

    assert_eq!(resp.status(), 200);

    let body = resp.text().await.unwrap();
    assert!(body.contains("rJMX-Exporter"));
}
```

### 7.3 수동 테스트

```bash
# 1. 빌드 테스트
cargo build

# 2. 단위 테스트 실행
cargo test

# 3. 서버 시작
cargo run

# 4. 엔드포인트 테스트
curl http://localhost:9090/
curl http://localhost:9090/health
curl http://localhost:9090/metrics

# 5. Docker 환경 테스트
docker-compose -f examples/docker-compose.yaml up -d
curl http://localhost:8778/jolokia/version
docker-compose -f examples/docker-compose.yaml down
```

### 7.4 성능 테스트

```bash
# 메모리 사용량 확인
/usr/bin/time -v ./target/release/rjmx-exporter

# 시작 시간 측정
hyperfine './target/release/rjmx-exporter --help'

# 요청 지연 시간 측정
ab -n 1000 -c 10 http://localhost:9090/health
```

---

## 8. 완료 기준 (Definition of Done)

### 8.1 코드 품질

- [ ] `cargo build` 성공 (경고 없음)
- [ ] `cargo build --release` 성공
- [ ] `cargo test` 모든 테스트 통과
- [ ] `cargo clippy -- -D warnings` 경고 없음
- [ ] `cargo fmt -- --check` 포맷 준수
- [ ] 모든 `pub` 아이템에 문서 주석 작성

### 8.2 기능 요구사항

- [ ] `GET /` 엔드포인트 동작
- [ ] `GET /health` 엔드포인트 동작 (JSON 응답)
- [ ] `GET /metrics` 엔드포인트 동작 (스텁 메트릭)
- [ ] 설정 파일 로딩 (YAML)
- [ ] CLI 인자 파싱 (`--config`, `--port`)
- [ ] Graceful shutdown (SIGTERM/SIGINT)

### 8.3 Docker 환경

- [ ] `docker-compose up` 성공
- [ ] Java 앱 정상 시작
- [ ] Jolokia 엔드포인트 접근 가능
- [ ] `curl http://localhost:8778/jolokia/version` 성공

### 8.4 문서화

- [ ] README.md 업데이트 (빌드/실행 방법)
- [ ] CLAUDE.md 상태 업데이트
- [ ] 코드 내 문서 주석 완료

---

## 9. 예상 산출물 (Expected Deliverables)

### 9.1 소스 코드

| 파일 | 설명 | 예상 라인 수 |
|------|------|--------------|
| `Cargo.toml` | 프로젝트 매니페스트 | ~50 |
| `src/main.rs` | 진입점 | ~50 |
| `src/lib.rs` | 라이브러리 루트 | ~30 |
| `src/config.rs` | 설정 관리 | ~150 |
| `src/error.rs` | 에러 타입 | ~50 |
| `src/server/mod.rs` | 서버 모듈 | ~80 |
| `src/server/handlers.rs` | 핸들러 | ~60 |
| `src/collector/mod.rs` | 수집기 스텁 | ~20 |
| `src/transformer/mod.rs` | 변환기 스텁 | ~20 |

### 9.2 Docker 파일

| 파일 | 설명 |
|------|------|
| `examples/docker-compose.yaml` | 개발 환경 구성 |
| `docker/java-app/Dockerfile` | Java 앱 이미지 |
| `docker/java-app/App.java` | 샘플 Java 앱 |
| `docker/prometheus/prometheus.yml` | Prometheus 설정 |

### 9.3 테스트

| 파일 | 설명 |
|------|------|
| `tests/integration/server_test.rs` | 서버 통합 테스트 |

### 9.4 설정 예시

| 파일 | 설명 |
|------|------|
| `config.example.yaml` | 설정 파일 예시 |

---

## 부록 A: 의존성 버전 정보

| 크레이트 | 버전 | 용도 |
|----------|------|------|
| tokio | 1.35 | 비동기 런타임 |
| axum | 0.7 | HTTP 서버 |
| reqwest | 0.11 | HTTP 클라이언트 |
| serde | 1.0 | 직렬화/역직렬화 |
| serde_yaml | 0.9 | YAML 파싱 |
| serde_json | 1.0 | JSON 파싱 |
| tracing | 0.1 | 구조화된 로깅 |
| tracing-subscriber | 0.3 | 로그 출력 |
| thiserror | 1.0 | 에러 타입 매크로 |
| anyhow | 1.0 | 에러 처리 유틸리티 |
| clap | 4.4 | CLI 인자 파싱 |
| tower-http | 0.5 | HTTP 미들웨어 |

---

## 부록 B: 참고 자료

- [Jolokia Protocol](https://jolokia.org/reference/html/protocol.html)
- [Prometheus Exposition Format](https://prometheus.io/docs/instrumenting/exposition_formats/)
- [Axum Documentation](https://docs.rs/axum/latest/axum/)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)
- [Rust Error Handling](https://doc.rust-lang.org/book/ch09-00-error-handling.html)

---

## 부록 C: 예상 일정

| 작업 | 예상 소요 시간 |
|------|----------------|
| Cargo.toml 및 기본 구조 | 1시간 |
| config.rs / error.rs | 2시간 |
| server 모듈 | 2시간 |
| Docker 환경 | 2시간 |
| 테스트 작성 | 2시간 |
| 문서화 및 정리 | 1시간 |
| **총 예상 시간** | **10시간** |

---

---

## 문서 끝
