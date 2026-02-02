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
    let bind_address = config.server.bind_address.clone();
    let metrics_path = config.server.path.clone();

    let state = AppState {
        config: Arc::new(config),
    };

    // Build router with configurable metrics path
    let app = Router::new()
        .route("/", get(handlers::root))
        .route("/health", get(handlers::health))
        .route(&metrics_path, get(handlers::metrics))
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    // Parse bind address from config
    let bind_addr: std::net::IpAddr = bind_address
        .parse()
        .map_err(|e| anyhow::anyhow!("Invalid bind_address '{}': {}", bind_address, e))?;
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
