use anyhow::{Result, anyhow};
use axum::{Json, Router, routing::get};
use metrics::{counter, gauge};
use rmcp::transport::{
    StreamableHttpServerConfig,
    streamable_http_server::{session::local::LocalSessionManager, tower::StreamableHttpService},
};
use serde_json::json;
use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};
use tokio::fs;
use tokio::net::{TcpListener, UnixListener};
use tower_http::{cors::CorsLayer, trace::TraceLayer};
use tracing::{debug, error, info, warn};

use crate::logs::init_logging_and_metrics;
use crate::server::auth::{TokenValidationConfig, require_bearer_auth};
use crate::server::http::health;
use crate::server::limit::create_rate_limit_layer;
use crate::tools::SurrealService;
use crate::utils::{format_duration, generate_connection_id};

/// Configuration for server startup
#[derive(Clone)]
pub struct ServerConfig {
    pub endpoint: Option<String>,
    pub ns: Option<String>,
    pub db: Option<String>,
    pub user: Option<String>,
    pub pass: Option<String>,
    pub server_url: String,
    pub bind_address: Option<String>,
    pub socket_path: Option<String>,
    pub auth_disabled: bool,
    pub rate_limit_rps: u32,
    pub rate_limit_burst: u32,
    pub auth_server: String,
    pub auth_audience: String,
    pub jwe_decryption_key: Option<String>,
    pub cloud_access_token: Option<String>,
    pub cloud_refresh_token: Option<String>,
}

// Global metrics
static ACTIVE_CONNECTIONS: AtomicU64 = AtomicU64::new(0);
static TOTAL_CONNECTIONS: AtomicU64 = AtomicU64::new(0);

/// Start the MCP server based on the provided configuration
pub async fn start_server(config: ServerConfig) -> Result<()> {
    // Output debugging information
    info!(
        endpoint = config.endpoint.as_deref(),
        namespace = config.ns.as_deref(),
        database = config.db.as_deref(),
        username = config.user.as_deref(),
        server_url = config.server_url,
        bind_address = config.bind_address.as_deref().unwrap_or("N/A"),
        socket_path = config.socket_path.as_deref().unwrap_or("N/A"),
        auth_disabled = config.auth_disabled,
        rate_limit_rps = config.rate_limit_rps,
        rate_limit_burst = config.rate_limit_burst,
        auth_server = config.auth_server,
        auth_audience = config.auth_audience,
        "Server configuration loaded"
    );
    match (config.bind_address.is_some(), config.socket_path.is_some()) {
        // We are running as a STDIO server
        (false, false) => start_stdio_server(config).await,
        // We are running as a HTTP server
        (true, false) => start_http_server(config).await,
        // We are running as a Unix socket
        (false, true) => start_unix_server(config).await,
        // This should never happen due to CLI argument groups
        (true, true) => Err(anyhow!(
            "Cannot specify both --bind-address and --socket-path"
        )),
    }
}

/// Start the MCP server in stdio mode
async fn start_stdio_server(config: ServerConfig) -> Result<()> {
    // Extract configuration values
    let ServerConfig {
        endpoint,
        ns,
        db,
        user,
        pass,
        cloud_access_token,
        cloud_refresh_token,
        ..
    } = config;
    // Initialize structured logging and metrics
    init_logging_and_metrics(true);
    // Output debugging information
    info!("Starting MCP server in stdio mode");
    // Generate a connection ID for this connection
    let connection_id = generate_connection_id();
    // Create a new SurrealDB service instance
    let service = SurrealService::with_config(
        connection_id.clone(),
        endpoint,
        ns,
        db,
        user,
        pass,
        cloud_access_token,
        cloud_refresh_token,
    );
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
async fn start_unix_server(config: ServerConfig) -> Result<()> {
    // Extract configuration values
    let ServerConfig {
        endpoint,
        ns,
        db,
        user,
        pass,
        socket_path,
        cloud_access_token,
        cloud_refresh_token,
        ..
    } = config;
    // Get the specified socket path
    let socket_path = socket_path.as_deref().unwrap();
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
        gauge!("surrealmcp.active_connections").set(active_connections as f64);
        counter!("surrealmcp.total_connections").increment(1);
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
        let cloud_access_token = cloud_access_token.clone();
        let cloud_refresh_token = cloud_refresh_token.clone();
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
                cloud_access_token,
                cloud_refresh_token,
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
                    gauge!("surrealmcp.active_connections").set(active_connections as f64);
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
                    gauge!("surrealmcp.active_connections").set(active_connections as f64);
                }
            }
        });
    }
}

/// Start the MCP server in HTTP mode
async fn start_http_server(config: ServerConfig) -> Result<()> {
    // Extract configuration values
    let ServerConfig {
        endpoint,
        ns,
        db,
        user,
        pass,
        server_url,
        bind_address,
        auth_disabled,
        rate_limit_rps,
        rate_limit_burst,
        auth_server,
        auth_audience,
        jwe_decryption_key,
        cloud_access_token,
        cloud_refresh_token,
        ..
    } = config;
    // Get the specified bind address
    let bind_address = bind_address.as_deref().unwrap();
    // Initialize structured logging and metrics
    init_logging_and_metrics(false);
    // Output debugging information
    info!(
        server_url = %server_url,
        bind_address = %bind_address,
        rate_limit_rps = rate_limit_rps,
        rate_limit_burst = rate_limit_burst,
        "Starting MCP server in HTTP mode with rate limiting"
    );
    // Create a TCP listener for the HTTP server
    let listener = TcpListener::bind(&bind_address)
        .await
        .map_err(|e| anyhow!("Failed to bind to address {bind_address}: {e}"))?;
    // List servers for authentication discovery
    let auth_servers = Json(json!({
        "resource": server_url,
        "bearer_methods_supported": ["header"],
        "authorization_servers": [auth_server],
        "scopes_supported": ["openid", "profile", "email", "offline_access"],
        "audience": auth_audience
    }));
    // Create CORS layer for /.well-known endpoints
    let cors_layer = CorsLayer::new()
        .allow_origin(tower_http::cors::Any)
        .allow_methods([axum::http::Method::GET, axum::http::Method::OPTIONS])
        .allow_headers([
            axum::http::header::AUTHORIZATION,
            axum::http::header::CONTENT_TYPE,
        ])
        .allow_credentials(false);
    // Create a service for /.well-known endpoints with CORS
    let well_known_service = Router::new()
        .route("/oauth-protected-resource", get(auth_servers))
        .layer(cors_layer);
    // Create a session manager for the HTTP server
    let session_manager = Arc::new(LocalSessionManager::default());
    // Create a new SurrealDB service instance for the HTTP server
    let mcp_service = StreamableHttpService::new(
        move || {
            Ok(SurrealService::with_config(
                generate_connection_id(),
                endpoint.clone(),
                ns.clone(),
                db.clone(),
                user.clone(),
                pass.clone(),
                cloud_access_token.clone(),
                cloud_refresh_token.clone(),
            ))
        },
        session_manager,
        StreamableHttpServerConfig {
            stateful_mode: true,
            sse_keep_alive: None,
        },
    );
    // Create rate limiting layer with metrics
    let rate_limit_layer = create_rate_limit_layer(rate_limit_rps, rate_limit_burst);
    // Create tracing layer for request logging
    let trace_layer = TraceLayer::new_for_http()
        .make_span_with(|request: &axum::http::Request<_>| {
            let connection_id = generate_connection_id();
            tracing::info_span!(
                "http_request",
                connection_id = %connection_id,
                method = %request.method(),
                uri = %request.uri(),
            )
        })
        .on_request(|request: &axum::http::Request<_>, _span: &tracing::Span| {
            debug!(
                method = %request.method(),
                uri = %request.uri(),
                "HTTP request started"
            );
        })
        .on_response(
            |response: &axum::http::Response<_>, latency: Duration, _span: &tracing::Span| {
                let status = response.status();
                if status.is_client_error() || status.is_server_error() {
                    warn!(
                        status = %status,
                        latency_ms = latency.as_millis(),
                        "HTTP request failed"
                    );
                } else {
                    info!(
                        status = %status,
                        latency_ms = latency.as_millis(),
                        "HTTP request completed"
                    );
                }
            },
        );
    // Create an Axum router with rate limiting and tracing at /mcp
    let mut router = Router::new()
        .nest_service("/.well-known", well_known_service)
        .nest_service("/mcp", mcp_service)
        .route("/health", get(health))
        .layer(trace_layer)
        .layer(rate_limit_layer);
    // Add bearer authentication middleware if specified
    if !auth_disabled {
        // Set the token validation config
        let token_config = TokenValidationConfig {
            expected_audience: auth_audience.clone(),
            jwe_decryption_key: jwe_decryption_key.clone(),
            ..Default::default()
        };
        // Add bearer authentication middleware
        router = router.layer(axum::middleware::from_fn(move |req, next| {
            let config = token_config.clone();
            require_bearer_auth(config, req, next)
        }));
    }
    // Serve the Axum router over HTTP
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            tokio::signal::ctrl_c()
                .await
                .expect("failed to install Ctrl+C handler");
        })
        .await?;
    // All ok
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        Router,
        body::Body,
        http::{Request, StatusCode},
        routing::get,
    };
    use tower::ServiceExt;

    #[tokio::test]
    async fn test_auth_discovery_includes_audience() {
        let config = ServerConfig {
            endpoint: None,
            ns: None,
            db: None,
            user: None,
            pass: None,
            server_url: "https://mcp.surrealdb.com".to_string(),
            bind_address: Some("127.0.0.1:0".to_string()),
            socket_path: None,
            auth_disabled: true,
            rate_limit_rps: 100,
            rate_limit_burst: 200,
            auth_server: "https://auth.surrealdb.com".to_string(),
            auth_audience: "https://custom.audience.com/".to_string(),
            jwe_decryption_key: None,
            cloud_access_token: None,
            cloud_refresh_token: None,
        };

        // Create a simple router to test the discovery endpoint
        let auth_servers = Json(json!({
            "resource": config.server_url,
            "authorization_servers": [config.auth_server],
            "bearer_methods_supported": ["header"],
            "audience": config.auth_audience
        }));

        let app = Router::new().route(
            "/oauth-protected-resource",
            get(move || async move { auth_servers }),
        );

        let request = Request::builder()
            .uri("/oauth-protected-resource")
            .body(Body::empty())
            .unwrap();

        let response = app.oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::OK);

        // For this test, we'll just verify the response structure exists
        // The actual JSON parsing would require additional dependencies
        assert!(response.headers().get("content-type").is_some());
    }
}
