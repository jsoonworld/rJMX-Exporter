//! rJMX-Exporter - High-performance JMX metrics exporter
//!
//! This binary provides a Prometheus-compatible metrics endpoint
//! that collects JMX metrics from Java applications via Jolokia.

use anyhow::Result;
use clap::Parser;
use tracing::info;

use rjmx_exporter::{config::Config, server};

/// rJMX-Exporter CLI arguments
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Args {
    /// Path to configuration file
    #[arg(short, long, default_value = "config.yaml")]
    config: String,

    /// Server port (overrides config file)
    #[arg(short, long, env = "RJMX_PORT")]
    port: Option<u16>,

    /// Log level (trace, debug, info, warn, error)
    #[arg(long, default_value = "info", env = "RJMX_LOG_LEVEL")]
    log_level: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse CLI arguments
    let args = Args::parse();

    // Initialize logging
    rjmx_exporter::init_logging(&args.log_level)?;

    info!(
        version = env!("CARGO_PKG_VERSION"),
        "Starting rJMX-Exporter"
    );

    // Load configuration
    let config = Config::load_or_default(&args.config)?;
    let port = args.port.unwrap_or(config.server.port);

    // Start server
    server::run(config, port).await?;

    Ok(())
}
