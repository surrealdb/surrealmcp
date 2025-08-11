use anyhow::{Result, anyhow};
use surrealdb::{Surreal, engine::any, engine::any::Any, opt::auth::Root};
use tracing::{debug, instrument};

/// Create a new SurrealDB connection for a client
#[instrument(skip(username, password, namespace, database), fields(url = %url))]
pub async fn create_client_connection(
    url: &str,
    username: Option<&str>,
    password: Option<&str>,
    namespace: Option<&str>,
    database: Option<&str>,
) -> Result<Surreal<Any>, anyhow::Error> {
    // Output debugging information
    debug!("Attempting to connect to SurrealDB");
    // Connect to SurrealDB using the Any engine
    let instance = any::connect(url)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    // Output debugging information
    debug!("Successfully connected to SurrealDB instance");
    // Attempt to authenticate if specified
    if let (Some(username), Some(password)) = (username, password) {
        debug!("Attempting authentication with username: {}", username);
        instance
            .signin(Root { username, password })
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
        debug!("Authentication successful");
    } else {
        debug!("No authentication credentials provided");
    }
    // Set namespace if provided
    if let Some(ns) = namespace {
        debug!("Setting namespace: {}", ns);
        instance
            .use_ns(ns)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
    }
    // Set database if provided
    if let Some(db) = database {
        debug!("Setting database: {}", db);
        instance
            .use_db(db)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
    }
    // Output debugging information
    debug!("Successfully established SurrealDB connection");
    // Return the instance
    Ok(instance)
}

/// Create a new SurrealDB connection for a client using a token
#[instrument(skip(token, namespace, database), fields(url = %url))]
pub async fn create_client_connection_with_token(
    url: &str,
    token: &str,
    _username: Option<&str>,
    _password: Option<&str>,
    namespace: Option<&str>,
    database: Option<&str>,
) -> Result<Surreal<Any>, anyhow::Error> {
    // Output debugging information
    debug!("Attempting to connect to SurrealDB with token");
    // Connect to SurrealDB using the Any engine
    let instance = any::connect(url)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    // Output debugging information
    debug!("Successfully connected to SurrealDB instance");
    // Authenticate with the token
    debug!("Attempting authentication with token");
    instance
        .authenticate(token)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    debug!("Authentication successful");
    // Set namespace if provided
    if let Some(ns) = namespace {
        debug!("Setting namespace: {}", ns);
        instance
            .use_ns(ns)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
    }
    // Set database if provided
    if let Some(db) = database {
        debug!("Setting database: {}", db);
        instance
            .use_db(db)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
    }
    // Output debugging information
    debug!("Successfully established SurrealDB connection with token");
    // Return the instance
    Ok(instance)
}
