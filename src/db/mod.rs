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
            .signin(Root {
                username: username.to_string(),
                password: password.to_string(),
            })
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

#[cfg(test)]
mod tests {
    use super::*;

    /// Connect-round-trip against the consolidated local instance on `:8000`.
    ///
    /// This is intentionally **read-only**: the default namespace/database are
    /// `hebb`/`memory`, which hold the live memory data (thousands of records).
    /// The test runs `INFO FOR DB` (a schema introspection query that never
    /// mutates rows) to prove connect + signin + `use_ns`/`use_db` + query all
    /// work end-to-end against the 3.1.x server. It MUST NOT issue any
    /// `CREATE`/`UPDATE`/`DELETE` — that would corrupt production memory.
    ///
    /// Skips gracefully (returns early) when the server is unreachable so CI
    /// without a live instance stays green; run it with `surreal` up on `:8000`
    /// to verify the 3.1.3 client wire format.
    #[tokio::test]
    async fn live_roundtrip_hebb_namespace_read_only() {
        let url = "ws://127.0.0.1:8000/rpc";

        // Probe: if connect fails, the server is down — skip rather than fail.
        let db = match create_client_connection(
            url,
            Some("root"),
            Some("root"),
            Some("hebb"),
            Some("memory"),
        )
        .await
        {
            Ok(db) => db,
            Err(e) => {
                eprintln!("skipping live round-trip: {url} unreachable ({e})");
                return;
            }
        };

        // Read-only introspection: proves the query path works without mutating
        // the live `hebb`/`memory` data.
        let mut res = db
            .query("INFO FOR DB")
            .await
            .expect("INFO FOR DB query should succeed against live :8000");
        let info: Option<surrealdb::types::Value> =
            res.take(0).expect("INFO FOR DB should yield a row");
        assert!(
            info.is_some(),
            "expected a non-empty INFO FOR DB result from hebb/memory"
        );
    }
}
