//! HTTP server module
//!
//! Provides the Axum-based HTTP server for serving metrics.
//! Supports both HTTP and HTTPS (TLS) modes.

pub mod handlers;

use std::net::SocketAddr;
use std::path::Path;
use std::sync::Arc;

use anyhow::Result;
use axum::{routing::get, Router};
use axum_server::tls_rustls::RustlsConfig;
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
/// Starts either an HTTP or HTTPS server based on TLS configuration.
/// When TLS is enabled, loads certificates from the specified paths
/// and starts an HTTPS server. Otherwise, starts a plain HTTP server.
///
/// # Arguments
/// * `config` - Application configuration (with all overrides already applied)
///
/// # Errors
/// Returns an error if:
/// - The server fails to start
/// - TLS is enabled but certificate files cannot be loaded
pub async fn run(config: Config) -> Result<()> {
    let port = config.server.port;
    let bind_address = config.server.bind_address.clone();
    let metrics_path = config.server.path.clone();
    let tls_config = config.server.tls.clone();

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

    // Start server with or without TLS
    if tls_config.enabled {
        run_https_server(app, addr, &metrics_path, &tls_config).await
    } else {
        run_http_server(app, addr, &metrics_path).await
    }
}

/// Run a plain HTTP server
async fn run_http_server(app: Router, addr: SocketAddr, metrics_path: &str) -> Result<()> {
    info!(
        address = %addr,
        metrics_path = %metrics_path,
        tls = false,
        "Server listening (HTTP)"
    );

    let listener = tokio::net::TcpListener::bind(addr).await?;

    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown_signal())
        .await?;

    info!("Server shutdown complete");
    Ok(())
}

/// Run an HTTPS server with TLS
async fn run_https_server(
    app: Router,
    addr: SocketAddr,
    metrics_path: &str,
    tls_config: &crate::config::TlsConfig,
) -> Result<()> {
    // Get certificate and key file paths (already validated in config)
    let cert_file = tls_config
        .cert_file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("TLS cert_file is required when TLS is enabled"))?;
    let key_file = tls_config
        .key_file
        .as_ref()
        .ok_or_else(|| anyhow::anyhow!("TLS key_file is required when TLS is enabled"))?;

    // Validate that certificate files exist
    let cert_path = Path::new(cert_file);
    let key_path = Path::new(key_file);

    if !cert_path.exists() {
        return Err(anyhow::anyhow!(
            "TLS certificate file not found: {}",
            cert_file
        ));
    }
    if !key_path.exists() {
        return Err(anyhow::anyhow!(
            "TLS private key file not found: {}",
            key_file
        ));
    }

    // Load TLS configuration
    let rustls_config = RustlsConfig::from_pem_file(cert_path, key_path)
        .await
        .map_err(|e| anyhow::anyhow!("Failed to load TLS certificates: {}", e))?;

    info!(
        address = %addr,
        metrics_path = %metrics_path,
        tls = true,
        cert_file = %cert_file,
        "Server listening (HTTPS)"
    );

    // Create and run the HTTPS server with graceful shutdown
    let handle = axum_server::Handle::new();
    let shutdown_handle = handle.clone();

    // Spawn shutdown signal handler
    tokio::spawn(async move {
        shutdown_signal().await;
        shutdown_handle.graceful_shutdown(Some(std::time::Duration::from_secs(10)));
    });

    axum_server::bind_rustls(addr, rustls_config)
        .handle(handle)
        .serve(app.into_make_service())
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
