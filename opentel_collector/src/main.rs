//! OpenTelemetry Sidecar Collector Binary

use opentel_collector::{Config, SidecarCollector, Result};
use tracing::{info, error};
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};

#[tokio::main]
async fn main() -> Result<()> {
    // Initialize tracing
    initialize_tracing();

    info!("Starting OpenTelemetry Sidecar Collector v{}", env!("CARGO_PKG_VERSION"));

    // Load configuration
    let config = Config::from_env();

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    info!(
        "Collector configuration - Service: {}, Pod: {}, Namespace: {}, Gateway: {}",
        config.service_name,
        config.pod_name,
        config.namespace,
        config.gateway_url
    );

    // Create and start collector
    let collector = SidecarCollector::new(config)?;

    if let Err(e) = collector.start().await {
        error!("Collector failed: {}", e);
        std::process::exit(1);
    }

    Ok(())
}

/// Initialize structured logging
fn initialize_tracing() {
    let log_level = std::env::var("RUST_LOG").unwrap_or_else(|_| "info".to_string());

    let fmt_layer = tracing_subscriber::fmt::layer()
        .with_target(false)
        .with_thread_ids(false)
        .with_thread_names(false)
        .with_file(false)
        .with_line_number(false)
        .json();

    let filter_layer = tracing_subscriber::EnvFilter::try_from_default_env()
        .or_else(|_| tracing_subscriber::EnvFilter::try_new(&log_level))
        .unwrap();

    tracing_subscriber::registry()
        .with(filter_layer)
        .with(fmt_layer)
        .init();
}
