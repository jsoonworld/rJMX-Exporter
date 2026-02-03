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
use crate::metrics::internal_metrics;
use crate::transformer::PrometheusFormatter;

/// Sanitize URL for use in metric labels by removing credentials
///
/// Converts URLs like "http://user:pass@host:port/path" to "host:port"
fn sanitize_url_for_label(url: &str) -> String {
    // Try to parse as URL and extract host:port
    if let Ok(parsed) = url::Url::parse(url) {
        let host = parsed.host_str().unwrap_or("unknown");
        if let Some(port) = parsed.port() {
            format!("{}:{}", host, port)
        } else {
            host.to_string()
        }
    } else {
        // Fallback: try simple string manipulation
        // Remove scheme and extract after '@' if present
        let without_scheme = url
            .trim_start_matches("http://")
            .trim_start_matches("https://");
        let after_at = without_scheme.rsplit('@').next().unwrap_or(without_scheme);
        // Take only host:port part (before any path)
        after_at.split('/').next().unwrap_or(after_at).to_string()
    }
}

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
    let metrics_registry = internal_metrics();

    // Get target name from config for metrics labeling
    // Sanitize URL to remove credentials (user:pass@host -> host)
    let target_name = sanitize_url_for_label(&state.config.jolokia.url);

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
                    errors.push(format!("{}: status {}", mbean, response.status));
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
        Ok(metrics) => {
            // Record rule matches for internal metrics
            for rule in state.engine.rules().rules() {
                if rule.is_compiled() {
                    // Record rule activity (simplified - actual match counting would require
                    // more detailed tracking in the transform engine)
                    metrics_registry.record_rule_match(&rule.pattern);
                }
            }
            metrics
        }
        Err(e) => {
            warn!(error = %e, "Transform error");
            errors.push(format!("transform: {}", e));
            vec![]
        }
    };

    // Format output
    let formatter = PrometheusFormatter::new();
    let mut output = formatter.format(&prometheus_metrics);

    // Calculate scrape duration
    let scrape_duration = start.elapsed().as_secs_f64();

    // Record internal metrics for this scrape
    if errors.is_empty() {
        metrics_registry.record_scrape_success(&target_name, scrape_duration);
    } else {
        metrics_registry.record_scrape_failure(&target_name, scrape_duration);
    }

    // Add exporter info metrics
    output.push_str(&format!(
        r#"# HELP rjmx_exporter_info rJMX-Exporter information
# TYPE rjmx_exporter_info gauge
rjmx_exporter_info{{version="{}"}} 1
# HELP rjmx_exporter_scrape_duration_seconds Time spent scraping metrics
# TYPE rjmx_exporter_scrape_duration_seconds gauge
rjmx_exporter_scrape_duration_seconds {}
# HELP rjmx_exporter_scrape_errors Number of errors during last scrape
# TYPE rjmx_exporter_scrape_errors gauge
rjmx_exporter_scrape_errors {}
# HELP rjmx_exporter_metrics_scraped Number of metrics scraped
# TYPE rjmx_exporter_metrics_scraped gauge
rjmx_exporter_metrics_scraped {}
"#,
        env!("CARGO_PKG_VERSION"),
        scrape_duration,
        errors.len(),
        prometheus_metrics.len()
    ));

    // Append internal observability metrics
    output.push_str(&metrics_registry.format_prometheus());

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
