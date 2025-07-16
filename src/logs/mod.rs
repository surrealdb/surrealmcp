use metrics::{counter, gauge};
use tracing::info;
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

/// Initialize structured logging and metrics collection
pub fn init_logging_and_metrics(stdio: bool) {
    // Check if we are running in stdio mode
    if stdio {
        // Set up environment filter for log levels
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("surrealmcp=error,rmcp=error"));
        // Initialize tracing subscriber with stderr output
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_writer(std::io::stderr),
            )
            .init();
    } else {
        // Set up environment filter for log levels
        let filter = EnvFilter::try_from_default_env()
            .unwrap_or_else(|_| EnvFilter::new("surrealmcp=trace,rmcp=warn"));
        // Initialize tracing subscriber with stdout output
        tracing_subscriber::registry()
            .with(filter)
            .with(
                tracing_subscriber::fmt::layer()
                    .with_target(true)
                    .with_writer(std::io::stdout),
            )
            .init();
    }
    // Output debugging information
    info!("Logging and tracing initialized");
    // Initialize metrics with default values
    gauge!("surrealmcp.active_connections", 0.0);
    counter!("surrealmcp.total_connections", 0);
    counter!("surrealmcp.total_queries", 0);
    counter!("surrealmcp.total_errors", 0);
    counter!("surrealmcp.total_query_errors", 0);
    counter!("surrealmcp.total_rate_limit_errors", 0);
    // Output debugging information
    info!("Metrics collection initialized");
}
