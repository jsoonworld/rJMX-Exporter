//! Error types for rJMX-Exporter
//!
//! This module defines the error types used throughout the application.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use thiserror::Error;

/// Rule parsing and regex related errors
#[derive(Error, Debug)]
pub enum RuleError {
    /// Regex pattern compilation failed
    #[error("Invalid regex pattern '{pattern}': {source}")]
    InvalidPattern {
        pattern: String,
        #[source]
        source: regex::Error,
    },

    /// Unsupported regex syntax
    #[error("Unsupported regex syntax in pattern '{pattern}': {feature}")]
    UnsupportedSyntax { pattern: String, feature: String },

    /// Rule compilation failed (with index)
    #[error("Failed to compile rule at index {index}: {source}")]
    RuleCompileFailed {
        index: usize,
        #[source]
        source: Box<RuleError>,
    },
}

/// Transform engine errors
#[derive(Error, Debug)]
pub enum TransformError {
    /// Rule error
    #[error("Rule error: {0}")]
    Rule(#[from] RuleError),

    /// Invalid metric name
    #[error("Invalid metric name '{name}': {reason}")]
    InvalidMetricName { name: String, reason: String },

    /// Invalid label name
    #[error("Invalid label name '{name}': {reason}")]
    InvalidLabelName { name: String, reason: String },

    /// Missing capture group
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

/// Collector module error types
#[derive(Error, Debug)]
pub enum CollectorError {
    /// HTTP client initialization failed
    #[error("Failed to initialize HTTP client: {0}")]
    HttpClientInit(#[source] reqwest::Error),

    /// HTTP request failed
    #[error("HTTP request failed: {0}")]
    HttpRequest(#[source] reqwest::Error),

    /// HTTP response read failed
    #[error("Failed to read HTTP response: {0}")]
    HttpResponse(#[source] reqwest::Error),

    /// HTTP status code error
    #[error("HTTP error status: {0}")]
    HttpStatus(u16),

    /// JSON parsing error
    #[error("JSON parse error: {0}")]
    JsonParse(String),

    /// Jolokia error response
    #[error("Jolokia error (status {status}): {message}")]
    JolokiaError { status: u16, message: String },

    /// MBean not found
    #[error("MBean not found: {0}")]
    MBeanNotFound(String),

    /// Invalid ObjectName
    #[error("Invalid ObjectName: {0}")]
    InvalidObjectName(String),

    /// Timeout
    /// The value is the configured timeout in milliseconds, if known.
    #[error("Request timed out{}", .0.map(|ms| format!(" after {}ms", ms)).unwrap_or_default())]
    Timeout(Option<u64>),

    /// Connection failed
    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    /// Maximum retries exceeded
    #[error("Maximum retries exceeded")]
    MaxRetriesExceeded,

    /// Authentication failed
    #[error("Authentication failed")]
    AuthenticationFailed,
}

impl CollectorError {
    /// Check if the error is retryable
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

    /// Extract HTTP status code
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
