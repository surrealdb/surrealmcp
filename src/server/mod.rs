use anyhow::{Result, anyhow};
use axum::Router;
use metrics::{counter, gauge};
use rmcp::transport::{
    StreamableHttpServerConfig,
    streamable_http_server::{session::local::LocalSessionManager, tower::StreamableHttpService},
};
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use tokio::fs;
use tokio::net::{TcpListener, UnixListener};
use tracing::{debug, error, info};

use crate::logs::init_logging_and_metrics;
use crate::tools::SurrealService;
use crate::utils::{format_duration, generate_connection_id};

// Global metrics
static ACTIVE_CONNECTIONS: AtomicU64 = AtomicU64::new(0);
static TOTAL_CONNECTIONS: AtomicU64 = AtomicU64::new(0);

/// Start the MCP server based on the provided configuration
pub async fn start_server(
    endpoint: Option<String>,
    ns: Option<String>,
    db: Option<String>,
    user: Option<String>,
    pass: Option<String>,
    bind_address: Option<String>,
    socket_path: Option<String>,
) -> Result<()> {
    // Output debugging information
    info!(
        endpoint = endpoint.as_deref(),
        namespace = ns.as_deref(),
        database = db.as_deref(),
        username = user.as_deref(),
        bind_address = bind_address.as_deref(),
        socket_path = socket_path.as_deref(),
        "Server configuration loaded"
    );
    // Determine server mode based on arguments
    match (bind_address.as_ref(), socket_path.as_ref()) {
        // This should never happen due to CLI argument groups
        (Some(_), Some(_)) => Err(anyhow!(
            "Cannot specify both --bind-address and --socket-path"
        )),
        // We are running as a STDIO server
        (None, None) => start_stdio_server(endpoint, ns, db, user, pass).await,
        // We are running as a Unix socket
        (None, Some(path)) => start_unix_server(endpoint, ns, db, user, pass, path).await,
        // We are running as a HTTP server
        (Some(addr), None) => start_http_server(endpoint, ns, db, user, pass, addr).await,
    }
}

/// Start the MCP server in stdio mode
async fn start_stdio_server(
    endpoint: Option<String>,
    ns: Option<String>,
    db: Option<String>,
    user: Option<String>,
    pass: Option<String>,
) -> Result<()> {
    // Initialize structured logging and metrics
    init_logging_and_metrics(true);
    // Output debugging information
    info!("Starting MCP server in stdio mode");
    // Generate a connection ID for this connection
    let connection_id = generate_connection_id();
    // Create a new SurrealDB service instance
    let service = SurrealService::with_config(connection_id.clone(), endpoint, ns, db, user, pass);
    // Initialize the connection using startup configuration
    if let Err(e) = service.initialize_connection().await {
        error!(
            connection_id = %service.connection_id,
            error = %e,
            "Failed to initialize database connection"
        );
    }
    // Create an MCP server instance for stdin/stdout
    match rmcp::serve_server(service.clone(), (tokio::io::stdin(), tokio::io::stdout())).await {
        Ok(server) => {
            info!(
                connection_id = %service.connection_id,
                "MCP server instance creation succeeded"
            );
            // Wait for the server to complete its work
            let _ = server.waiting().await;
            info!(
                connection_id = %service.connection_id,
                "MCP server completed"
            );
        }
        Err(e) => {
            error!(
                connection_id = %service.connection_id,
                error = %e,
                "MCP server instance creation failed"
            );
            return Err(anyhow!(e));
        }
    }
    Ok(())
}

/// Start the MCP server in Unix socket mode
async fn start_unix_server(
    endpoint: Option<String>,
    ns: Option<String>,
    db: Option<String>,
    user: Option<String>,
    pass: Option<String>,
    socket_path: &str,
) -> Result<()> {
    // Initialize structured logging and metrics
    init_logging_and_metrics(false);
    // Get the specified socket path
    let socket_path = Path::new(socket_path);
    // Remove existing socket file if it exists
    if socket_path.exists() {
        fs::remove_file(socket_path).await?;
        info!(
            "Removed existing Unix socket file: {}",
            socket_path.display()
        );
    }
    // Create a Unix domain socket listener at the specified path
    let listener = UnixListener::bind(socket_path)?;
    // Log that the server is listening on the Unix socket
    info!(
        socket_path = %socket_path.display(),
        "Starting MCP server in Unix socket mode"
    );
    // Main server loop for Unix socket connections
    loop {
        // Accept incoming connections from the Unix socket
        let (stream, addr) = listener.accept().await?;
        // Generate a connection ID for this connection
        let connection_id = generate_connection_id();
        // Output debugging information
        info!(
            connection_id = %connection_id,
            peer_addr = ?addr,
            "New Unix socket connection accepted"
        );
        // Update connection metrics
        let active_connections = ACTIVE_CONNECTIONS.fetch_add(1, Ordering::SeqCst) + 1;
        let total_connections = TOTAL_CONNECTIONS.fetch_add(1, Ordering::SeqCst) + 1;
        gauge!("surrealmcp.active_connections", active_connections as f64);
        counter!("surrealmcp.total_connections", 1);
        // Output debugging information
        info!(
            connection_id = %connection_id,
            active_connections,
            total_connections,
            "Connection metrics updated"
        );
        // Clone configuration values for this connection
        let endpoint = endpoint.clone();
        let namespace = ns.clone();
        let database = db.clone();
        let user = user.clone();
        let pass = pass.clone();
        // Spawn a new async task to handle this client connection
        tokio::spawn(async move {
            let _span =
                tracing::info_span!("handle_unix_connection", connection_id = %connection_id);
            let _enter = _span.enter();

            debug!("Handling Unix socket connection");
            let service = SurrealService::with_config(
                connection_id.clone(),
                endpoint,
                namespace,
                database,
                user,
                pass,
            );
            // Initialize the connection using startup configuration only if endpoint is specified
            if let Err(e) = service.initialize_connection().await {
                error!(
                    connection_id = %service.connection_id,
                    error = %e,
                    "Failed to initialize database connection"
                );
            }
            // Create an MCP server instance for this connection
            match rmcp::serve_server(service.clone(), stream).await {
                Ok(server) => {
                    info!(
                        connection_id = %service.connection_id,
                        "MCP server instance creation succeeded"
                    );
                    // Wait for the server to complete its work
                    let _ = server.waiting().await;
                    // Update metrics when connection closes
                    let active_connections = ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::SeqCst) - 1;
                    gauge!("surrealmcp.active_connections", active_connections as f64);
                    // Output debugging information
                    info!(
                        connection_id = %service.connection_id,
                        connection_time = %format_duration(Instant::now() - service.connected_at),
                        active_connections,
                        "Connection closed"
                    );
                }
                Err(e) => {
                    // Output debugging information
                    error!(
                        connection_id = %service.connection_id,
                        error = %e,
                        "MCP server instance creation failed"
                    );
                    // Update metrics when connection fails
                    let active_connections = ACTIVE_CONNECTIONS.fetch_sub(1, Ordering::SeqCst) - 1;
                    gauge!("surrealmcp.active_connections", active_connections as f64);
                }
            }
        });
    }
}

/// Start the MCP server in HTTP mode
async fn start_http_server(
    endpoint: Option<String>,
    ns: Option<String>,
    db: Option<String>,
    user: Option<String>,
    pass: Option<String>,
    bind_address: &str,
) -> Result<()> {
    // Initialize structured logging and metrics
    init_logging_and_metrics(false);
    // Output debugging information
    info!(
        bind_address = %bind_address,
        "Starting MCP server in HTTP mode"
    );
    // Create a TCP listener for the HTTP server
    let listener = TcpListener::bind(bind_address).await?;
    // Create a session manager for the HTTP server
    let session_manager = Arc::new(LocalSessionManager::default());
    // Create a new SurrealDB service instance for the HTTP server
    let service = StreamableHttpService::new(
        move || {
            Ok(SurrealService::with_config(
                generate_connection_id(),
                endpoint.clone(),
                ns.clone(),
                db.clone(),
                user.clone(),
                pass.clone(),
            ))
        },
        session_manager,
        StreamableHttpServerConfig {
            stateful_mode: true,
            sse_keep_alive: None,
        },
    );
    // Create an Axum router at /mcp
    let router = Router::new().nest_service("/mcp", service);
    // Serve the Axum router over HTTP
    axum::serve(listener, router).await?;
    // All ok
    Ok(())
}
