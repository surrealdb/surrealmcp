use anyhow::Result;
use metrics::{counter, histogram};
use rmcp::model::Content;
use std::time::Instant;
use std::{collections::HashMap, time::Duration};
use surrealdb::{Surreal, Value, engine::any::Any};
use tracing::{debug, error, info};

/// Response from executing a SurrealDB query
#[derive(Debug)]
#[allow(dead_code)]
pub struct Response {
    /// Query ID for tracking
    pub query_id: u64,
    /// The query that was executed
    pub query: String,
    /// Duration of the query execution
    pub duration: Duration,
    /// Error message if the query failed
    pub error: Option<String>,
    /// The result of the query as a formatted string
    pub result: Option<surrealdb::Response>,
}

impl Response {
    /// Convert the response to an MCP Tool Result
    pub fn to_mcp_result(&self) -> Result<rmcp::model::CallToolResult, rmcp::ErrorData> {
        if let Some(res) = &self.result {
            Ok(rmcp::model::CallToolResult::success(vec![Content::text(
                format!("{res:?}"),
            )]))
        } else {
            let error_msg = self
                .error
                .as_ref()
                .unwrap_or(&"Unknown error".to_string())
                .clone();
            Err(rmcp::ErrorData::internal_error(error_msg, None))
        }
    }
}

/// Execute a SurrealQL query against the specified SurrealDB endpoint
///
/// This function executes a SurrealQL query against the provided SurrealDB client.
/// It handles parameter binding, query execution, and result formatting.
///
/// # Arguments
/// * `db` - The SurrealDB client instance
/// * `query_string` - The SurrealQL query to execute
/// * `parameters` - Optional parameters to bind to the query
/// * `query_id` - Unique identifier for tracking this query
/// * `connection_id` - Connection ID for logging purposes
///
/// # Returns
/// * `Result<Response, anyhow::Error>` - The query response or an error
pub async fn execute_query(
    db: &Surreal<Any>,
    query_id: u64,
    query_string: String,
    parameters: Option<HashMap<String, Value>>,
    connection_id: &str,
) -> Result<Response, anyhow::Error> {
    // Start the measurement timer
    let start_time = Instant::now();
    // Output debugging information
    debug!(
        connection_id = %connection_id,
        query_id,
        query_string = %query_string,
        "Executing SurrealQL query"
    );
    // Build the query string
    let mut query = db.query(&query_string);
    // Bind any parameters
    if let Some(params) = parameters {
        for (key, value) in params {
            query = query.bind((key, value));
        }
    }
    // Execute the query
    match query.await {
        Ok(res) => {
            // Get the duration of the query
            let duration = start_time.elapsed();
            // Output debugging information
            info!(
                connection_id = %connection_id,
                query_id,
                query = %query_string,
                duration_ms = duration.as_millis(),
                "Query execution succeeded"
            );
            // Update query metrics
            counter!("surrealmcp.total_queries").increment(1);
            histogram!("surrealmcp.query_duration_ms").record(duration.as_millis() as f64);
            // Return the response
            Ok(Response {
                query: query_string,
                result: Some(res),
                error: None,
                duration,
                query_id,
            })
        }
        Err(e) => {
            // Get the duration of the query
            let duration = start_time.elapsed();
            // Output debugging information
            error!(
                connection_id = %connection_id,
                query_id,
                query = %query_string,
                duration_ms = duration.as_millis(),
                error = %e,
                "Query execution failed"
            );
            // Update query metrics
            counter!("surrealmcp.total_query_errors").increment(1);
            histogram!("surrealmcp.query_duration_ms").record(duration.as_millis() as f64);
            // Return the response
            Ok(Response {
                query: query_string,
                result: None,
                error: Some(e.to_string()),
                duration,
                query_id,
            })
        }
    }
}
