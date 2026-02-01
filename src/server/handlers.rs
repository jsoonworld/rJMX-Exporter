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
    /// Health status
    status: String,
    /// Application version
    version: String,
}

/// Root endpoint - displays basic info
pub async fn root(State(state): State<AppState>) -> Html<String> {
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
        <li><a href="{}">Metrics</a></li>
    </ul>
</body>
</html>"#,
        env!("CARGO_PKG_VERSION"),
        state.config.server.path
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
pub async fn metrics(State(_state): State<AppState>) -> impl IntoResponse {
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
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; charset=utf-8",
        )],
        stub_metrics,
    )
}
