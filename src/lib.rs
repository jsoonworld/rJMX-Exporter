//! rJMX-Exporter library
//!
//! This crate provides the core functionality for collecting JMX metrics
//! from Java applications via Jolokia and exporting them in Prometheus format.

pub mod cli;
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
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new(level));

    tracing_subscriber::registry()
        .with(filter)
        .with(tracing_subscriber::fmt::layer())
        .try_init()
        .map_err(|e| anyhow::anyhow!("Failed to initialize logging: {}", e))?;

    Ok(())
}
