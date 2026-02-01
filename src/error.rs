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
