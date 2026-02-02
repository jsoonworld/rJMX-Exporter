//! HTTP request handlers
//!
//! Contains handlers for all HTTP endpoints.

use std::time::Instant;

use axum::{
    extract::State,
    http::StatusCode,
    response::{Html, IntoResponse},
    Json,
};
use serde::Serialize;
use tracing::{debug, instrument, warn};

use super::AppState;
use crate::transformer::PrometheusFormatter;

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

/// Default MBeans to collect when no whitelist is configured
const DEFAULT_MBEANS: &[&str] = &[
    "java.lang:type=Memory",
    "java.lang:type=Threading",
    "java.lang:type=ClassLoading",
    "java.lang:type=OperatingSystem",
    "java.lang:type=Runtime",
    "java.lang:type=GarbageCollector,*",
];

/// Metrics endpoint - collects JMX metrics via Jolokia and returns Prometheus format
#[instrument(skip(state), name = "metrics_handler")]
pub async fn metrics(State(state): State<AppState>) -> impl IntoResponse {
    let start = Instant::now();

    // Determine which MBeans to collect
    let mbeans_to_collect: Vec<String> = if !state.config.whitelist_object_names.is_empty() {
        state.config.whitelist_object_names.clone()
    } else {
        DEFAULT_MBEANS.iter().map(|s| s.to_string()).collect()
    };

    debug!(
        mbeans_count = mbeans_to_collect.len(),
        "Starting metrics collection"
    );

    // Collect metrics from Jolokia
    let mut all_responses = Vec::new();
    let mut errors = Vec::new();

    for mbean in &mbeans_to_collect {
        // Skip if in blacklist
        if state
            .config
            .blacklist_object_names
            .iter()
            .any(|b| mbean.contains(b))
        {
            debug!(mbean = %mbean, "Skipping blacklisted MBean");
            continue;
        }

        match state.client.read_mbean(mbean, None).await {
            Ok(response) => {
                if response.status == 200 {
                    all_responses.push(response);
                } else {
                    debug!(
                        mbean = %mbean,
                        status = response.status,
                        error = ?response.error,
                        "MBean returned non-200 status"
                    );
                }
            }
            Err(e) => {
                warn!(mbean = %mbean, error = %e, "Failed to collect MBean");
                errors.push(format!("{}: {}", mbean, e));
            }
        }
    }

    // Transform to Prometheus metrics
    let prometheus_metrics = match state.engine.transform(&all_responses) {
        Ok(metrics) => metrics,
        Err(e) => {
            warn!(error = %e, "Transform error");
            vec![]
        }
    };

    // Format output
    let formatter = PrometheusFormatter::new();
    let mut output = formatter.format(&prometheus_metrics);

    // Add exporter info metrics
    let scrape_duration = start.elapsed().as_secs_f64();
    output.push_str(&format!(
        r#"# HELP rjmx_exporter_info rJMX-Exporter information
# TYPE rjmx_exporter_info gauge
rjmx_exporter_info{{version="{}"}} 1
# HELP rjmx_exporter_scrape_duration_seconds Time spent scraping metrics
# TYPE rjmx_exporter_scrape_duration_seconds gauge
rjmx_exporter_scrape_duration_seconds {}
# HELP rjmx_exporter_scrape_errors_total Number of errors during scrape
# TYPE rjmx_exporter_scrape_errors_total counter
rjmx_exporter_scrape_errors_total {}
# HELP rjmx_exporter_metrics_scraped Number of metrics scraped
# TYPE rjmx_exporter_metrics_scraped gauge
rjmx_exporter_metrics_scraped {}
"#,
        env!("CARGO_PKG_VERSION"),
        scrape_duration,
        errors.len(),
        prometheus_metrics.len()
    ));

    debug!(
        duration_ms = start.elapsed().as_millis() as u64,
        metrics_count = prometheus_metrics.len(),
        errors_count = errors.len(),
        "Metrics collection complete"
    );

    (
        StatusCode::OK,
        [(
            axum::http::header::CONTENT_TYPE,
            "text/plain; version=0.0.4; charset=utf-8",
        )],
        output,
    )
}
