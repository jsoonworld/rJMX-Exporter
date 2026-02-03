//! Error types for rJMX-Exporter
//!
//! This module defines the error types used throughout the application.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

/// Rule 파싱 및 regex 관련 에러
#[derive(Error, Debug)]
pub enum RuleError {
    /// 정규식 패턴 컴파일 실패
    #[error("Invalid regex pattern '{pattern}': {source}")]
    InvalidPattern {
        pattern: String,
        #[source]
        source: regex::Error,
    },

    /// 지원되지 않는 regex 문법
    #[error("Unsupported regex syntax in pattern '{pattern}': {feature}")]
    UnsupportedSyntax { pattern: String, feature: String },

    /// 규칙 컴파일 실패 (인덱스 포함)
    #[error("Failed to compile rule at index {index}: {source}")]
    RuleCompileFailed {
        index: usize,
        #[source]
        source: Box<RuleError>,
    },
}

/// Transform 엔진 에러
#[derive(Error, Debug)]
pub enum TransformError {
    /// 규칙 에러
    #[error("Rule error: {0}")]
    Rule(#[from] RuleError),

    /// 유효하지 않은 메트릭명
    #[error("Invalid metric name '{name}': {reason}")]
    InvalidMetricName { name: String, reason: String },

    /// 유효하지 않은 라벨명
    #[error("Invalid label name '{name}': {reason}")]
    InvalidLabelName { name: String, reason: String },

    /// 캡처 그룹 누락
    #[error("Missing capture group ${group} in pattern")]
    MissingCaptureGroup { group: usize },
}

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
    Transform(#[from] TransformError),

    /// Internal server error
    #[error("Internal error: {0}")]
    Internal(String),

    /// Collector error
    #[error("Collector error: {0}")]
    Collector(#[from] CollectorError),
}

/// Collector 모듈 에러 타입
#[derive(Error, Debug)]
pub enum CollectorError {
    /// HTTP 클라이언트 초기화 실패
    #[error("Failed to initialize HTTP client: {0}")]
    HttpClientInit(#[source] reqwest::Error),

    /// HTTP 요청 실패
    #[error("HTTP request failed: {0}")]
    HttpRequest(#[source] reqwest::Error),

    /// HTTP 응답 읽기 실패
    #[error("Failed to read HTTP response: {0}")]
    HttpResponse(#[source] reqwest::Error),

    /// HTTP 상태 코드 에러
    #[error("HTTP error status: {0}")]
    HttpStatus(u16),

    /// JSON 파싱 에러
    #[error("JSON parse error: {0}")]
    JsonParse(String),

    /// Jolokia 에러 응답
    #[error("Jolokia error (status {status}): {message}")]
    JolokiaError { status: u16, message: String },

    /// MBean을 찾을 수 없음
    #[error("MBean not found: {0}")]
    MBeanNotFound(String),

    /// 잘못된 ObjectName
    #[error("Invalid ObjectName: {0}")]
    InvalidObjectName(String),

    /// 타임아웃
    /// The value is the configured timeout in milliseconds, if known.
    #[error("Request timed out{}", .0.map(|ms| format!(" after {}ms", ms)).unwrap_or_default())]
    Timeout(Option<u64>),

    /// 연결 실패
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// 최대 재시도 초과
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    /// 인증 실패
    #[error("Authentication failed")]
    AuthenticationFailed,
}

impl CollectorError {
    /// 재시도 가능한 에러인지 확인
    pub fn is_retryable(&self) -> bool {
        matches!(
            self,
            CollectorError::HttpRequest(_)
                | CollectorError::HttpResponse(_)
                | CollectorError::Timeout(..)
                | CollectorError::ConnectionFailed(_)
                | CollectorError::HttpStatus(500..=599)
        )
    }

    /// HTTP 상태 코드 추출
    pub fn http_status(&self) -> Option<u16> {
        match self {
            CollectorError::HttpStatus(code) => Some(*code),
            CollectorError::JolokiaError { status, .. } => Some(*status),
            _ => None,
        }
    }
}

impl From<reqwest::Error> for CollectorError {
    fn from(err: reqwest::Error) -> Self {
        if err.is_timeout() {
            // Timeout value is unknown when converting from reqwest::Error
            // because reqwest API doesn't expose the configured timeout duration.
            // Use CollectorError::timeout_with_duration() when the duration is known.
            CollectorError::Timeout(None)
        } else if err.is_connect() {
            CollectorError::ConnectionFailed(err.to_string())
        } else if err.is_request() {
            CollectorError::HttpRequest(err)
        } else {
            CollectorError::HttpResponse(err)
        }
    }
}

impl CollectorError {
    /// Create a Timeout error with known duration
    pub fn timeout_with_duration(ms: u64) -> Self {
        CollectorError::Timeout(Some(ms))
    }
}

impl IntoResponse for AppError {
    fn into_response(self) -> Response {
        let (status, public_message, log_message) = match self {
            AppError::Config(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Configuration error",
                e.to_string(),
            ),
            AppError::HttpClient(e) => (StatusCode::BAD_GATEWAY, "Upstream error", e),
            AppError::Jolokia(e) => (StatusCode::BAD_GATEWAY, "Upstream error", e),
            AppError::Transform(e) => (
                StatusCode::INTERNAL_SERVER_ERROR,
                "Transform error",
                e.to_string(),
            ),
            AppError::Internal(e) => (StatusCode::INTERNAL_SERVER_ERROR, "Internal error", e),
            AppError::Collector(e) => (StatusCode::BAD_GATEWAY, "Collector error", e.to_string()),
        };

        tracing::error!(status = %status, error = %log_message, "Request failed");

        (status, public_message).into_response()
    }
}

/// Result type alias for application errors
pub type AppResult<T> = Result<T, AppError>;
