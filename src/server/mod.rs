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

use crate::collector::JolokiaClient;
use crate::config::Config;
use crate::transformer::{MetricType, Rule, RuleSet, TransformEngine};

/// Application state shared across handlers
#[derive(Clone)]
pub struct AppState {
    /// Application configuration
    pub config: Arc<Config>,
    /// Jolokia HTTP client
    pub client: Arc<JolokiaClient>,
    /// Metric transformation engine
    pub engine: Arc<TransformEngine>,
}

/// Convert config rules to transformer RuleSet
fn config_to_ruleset(config: &Config) -> RuleSet {
    let rules: Vec<Rule> = config
        .rules
        .iter()
        .map(|r| {
            let rule_type = r.r#type.to_lowercase();
            let metric_type = match rule_type.as_str() {
                "gauge" => MetricType::Gauge,
                "counter" => MetricType::Counter,
                _ => {
                    tracing::warn!(
                        rule_type = %r.r#type,
                        rule_name = %r.name,
                        "Unknown metric type; defaulting to untyped"
                    );
                    MetricType::Untyped
                }
            };

            let mut rule = Rule::new(&r.pattern, &r.name, metric_type);

            if let Some(ref help) = r.help {
                rule = rule.with_help(help);
            }

            for (k, v) in &r.labels {
                rule = rule.with_label(k, v);
            }

            if let Some(ref value) = r.value {
                rule = rule.with_value(value);
            }

            if let Some(factor) = r.value_factor {
                rule = rule.with_value_factor(factor);
            }

            rule
        })
        .collect();

    RuleSet::from_rules(rules)
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
    let bind_address = config.server.bind_address.clone();
    let metrics_path = config.server.path.clone();

    // Create Jolokia client
    let mut client = JolokiaClient::new(&config.jolokia.url, config.jolokia.timeout_ms)?;
    if let (Some(ref username), Some(ref password)) =
        (&config.jolokia.username, &config.jolokia.password)
    {
        client = client.with_auth(username, password);
    }

    // Create transform engine with rules from config
    let ruleset = config_to_ruleset(&config);
    ruleset.compile_all()?;

    let engine = TransformEngine::new(ruleset)
        .with_lowercase_names(config.lowercase_output_name)
        .with_lowercase_labels(config.lowercase_output_label_names);

    let state = AppState {
        config: Arc::new(config),
        client: Arc::new(client),
        engine: Arc::new(engine),
    };

    // Build router with configurable metrics path
    let app = Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health))
        .route(&metrics_path, get(handlers::metrics))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Parse bind address from config
    // Handle "localhost" specially, otherwise parse as IP address
    let bind_addr: std::net::IpAddr = if bind_address == "localhost" {
        std::net::IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)
    } else {
        bind_address
            .parse()
            .map_err(|e| anyhow::anyhow!("Invalid bind_address '{}': {}. Use an IP address (e.g., '0.0.0.0', '127.0.0.1') or 'localhost'.", bind_address, e))?
    };
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
