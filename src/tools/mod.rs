use anyhow::Result;
use metrics::{counter, histogram};
use rmcp::{
    Error as McpError, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use surrealdb::{Surreal, engine::any::Any};
use tokio::sync::Mutex;
use tracing::{debug, error, info, warn};

use crate::db;

// Global metrics
static QUERY_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize, schemars::JsonSchema)]
pub struct QueryParams {
    #[schemars(description = "The SurrealQL query string")]
    pub query: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateParams {
    #[schemars(description = "The table name or record ID where the new record will be created.")]
    pub table: String,
    #[schemars(description = "The JSON data to be inserted as the record content.")]
    pub data: serde_json::Value,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SelectParams {
    #[schemars(description = "The table name, record ID, or complex query to select from.")]
    pub thing: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateParams {
    #[schemars(description = "The table name, record ID, or complex query to update.")]
    pub thing: String,
    #[schemars(description = "The JSON data to apply to the record.")]
    pub data: serde_json::Value,
    #[schemars(description = "Update mode for applying data to existing records.")]
    pub update_mode: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeleteParams {
    #[schemars(description = "The table name, record ID, or complex query to delete.")]
    pub thing: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct RelateParams {
    #[schemars(description = "The source record ID in 'table:id' format.")]
    pub from_id: String,
    #[schemars(
        description = "The type of relationship that describes the connection between records."
    )]
    pub relationship_type: String,
    #[schemars(description = "The target record ID in 'table:id' format.")]
    pub to_id: String,
    #[schemars(description = "Optional JSON data to store on the relationship edge.")]
    pub content: Option<serde_json::Value>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CloudOrganizationParams {
    #[schemars(description = "ID of the SurrealDB Cloud organization")]
    pub organization_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CloudInstanceParams {
    #[schemars(description = "ID of the SurrealDB Cloud instance")]
    pub instance_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateCloudInstanceParams {
    #[schemars(description = "Name of the SurrealDB Cloud instance")]
    pub name: String,
    #[schemars(description = "ID of the SurrealDB Cloud organization")]
    pub organization_id: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct ConnectParams {
    #[schemars(description = "The SurrealDB endpoint URL.")]
    pub endpoint: String,
    #[schemars(description = "The namespace to use for organizing data.")]
    pub namespace: Option<String>,
    #[schemars(description = "The database name within the namespace.")]
    pub database: Option<String>,
    #[schemars(description = "Username for authentication.")]
    pub username: Option<String>,
    #[schemars(description = "Password for authentication.")]
    pub password: Option<String>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UseNamespaceParams {
    #[schemars(description = "The namespace to switch to.")]
    pub namespace: String,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UseDatabaseParams {
    #[schemars(description = "The database to switch to.")]
    pub database: String,
}

#[derive(Clone)]
pub struct SurrealService {
    /// The SurrealDB client instance to use for database operations
    pub db: Arc<Mutex<Option<Surreal<Any>>>>,
    /// Connection ID for tracking this client session
    pub connection_id: String,
    /// The configured SurrealDB endpoint URL (optionally set at server startup)
    pub endpoint: Option<String>,
    /// The configured SurrealDB namespace (optionally set at server startup)
    pub namespace: Option<String>,
    /// The configured SurrealDB database (optionally set at server startup)
    pub database: Option<String>,
    /// The configured SurrealDB username (optionally set at server startup)
    pub user: Option<String>,
    /// The configured SurrealDB password (optionally set at server startup)
    pub pass: Option<String>,
    /// Timestamp when this connection was established
    pub connected_at: std::time::Instant,
    /// Router containing all available tools
    pub tool_router: ToolRouter<Self>,
}

#[tool_router]
impl SurrealService {
    /// Create a new SurrealService instance with the provided database connection.
    ///
    /// This function initializes a new SurrealService instance that can be used
    /// to interact with a SurrealDB database. The database connection is provided
    /// as a SurrealDB client instance, which is used to execute queries and
    /// perform database operations.
    ///
    /// # Arguments
    /// * `connection_id` - Connection ID for tracking this session
    #[allow(dead_code)]
    pub fn new(connection_id: String) -> Self {
        // Output debugging information
        info!(connection_id = %connection_id, "Creating new client session");
        // Create a new service instance
        Self {
            db: Arc::new(Mutex::new(None)),
            connection_id,
            endpoint: None,
            namespace: None,
            database: None,
            user: None,
            pass: None,
            connected_at: Instant::now(),
            tool_router: Self::tool_router(),
        }
    }

    /// Create a new SurrealService instance with startup configuration.
    ///
    /// This function initializes a new SurrealService instance with predefined
    /// configuration that restricts what endpoints, namespaces, and databases
    /// can be used during the session.
    ///
    /// # Arguments
    /// * `connection_id` - Connection ID for tracking this session
    /// * `endpoint` - The SurrealDB endpoint URL (optional)
    /// * `namespace` - The namespace to use (optional)
    /// * `database` - The database to use (optional)
    /// * `user` - Username for authentication (optional)
    /// * `pass` - Password for authentication (optional)
    pub fn with_config(
        connection_id: String,
        endpoint: Option<String>,
        namespace: Option<String>,
        database: Option<String>,
        user: Option<String>,
        pass: Option<String>,
    ) -> Self {
        // Output debugging information
        info!(
            connection_id = %connection_id,
            endpoint = endpoint.as_deref(),
            namespace = namespace.as_deref(),
            database = database.as_deref(),
            "Creating new client session with config"
        );
        // Create a new service instance
        Self {
            db: Arc::new(Mutex::new(None)),
            connection_id,
            endpoint,
            namespace,
            database,
            user,
            pass,
            connected_at: Instant::now(),
            tool_router: Self::tool_router(),
        }
    }

    /// Execute a raw SurrealQL query against the database.
    ///
    /// This function allows you to run any valid SurrealQL query string directly.
    /// The query is executed on the configured database connection as-is without
    /// any preprocessing or validation. The query results are returned as text,
    /// or an error occurs if the query execution fails.
    ///
    /// # Arguments
    /// * `query_string` - The raw SurrealQL query to execute
    #[tool(description = r#"
Execute a raw SurrealQL query against the database.

This function allows you to run any valid SurrealQL query string directly. The query 
is executed on the configured database connection as-is without any preprocessing 
or validation. Use this for complex queries, custom logic, or operations not covered 
by the convenience methods.

The query results are returned as text, or an error occurs if the query execution fails.

Examples:
- SELECT * FROM person
- CREATE person:john CONTENT {name: "John", age: 30}
- UPDATE person SET age += 1 WHERE age < 30
- DELETE person WHERE age < 18
- RELATE person:john->knows->person:jane
"#)]
    pub async fn query(&self, params: Parameters<QueryParams>) -> Result<CallToolResult, McpError> {
        let QueryParams {
            query: query_string,
        } = params.0;
        let start_time = Instant::now();
        let query_id = QUERY_COUNTER.fetch_add(1, Ordering::SeqCst);
        // Output debugging information
        debug!(
            connection_id = %self.connection_id,
            query_id,
            query = %query_string,
            "Executing SurrealQL query"
        );
        // Lock the database connection
        let db_guard = self.db.lock().await;
        // Match the database connection
        match &*db_guard {
            Some(db) => {
                match db.query(&query_string).await {
                    Ok(res) => {
                        // Get the duration of the query
                        let duration = start_time.elapsed();
                        // Format the result as text
                        let text = format!("{res:?}");
                        // Output debugging information
                        info!(
                            connection_id = %self.connection_id,
                            query_id,
                            query = %query_string,
                            duration_ms = duration.as_millis(),
                            result_length = text.len(),
                            "Query execution succeeded"
                        );
                        // Update the total queries metric
                        counter!("surrealmcp.total_queries", 1);
                        // Update the query duration metric
                        histogram!("surrealmcp.query_duration_ms", duration.as_millis() as f64);
                        // Return success message
                        Ok(CallToolResult::success(vec![Content::text(text)]))
                    }
                    Err(e) => {
                        // Get the duration of the query
                        let duration = start_time.elapsed();
                        // Output debugging information
                        error!(
                            connection_id = %self.connection_id,
                            query_id,
                            query = %query_string,
                            duration_ms = duration.as_millis(),
                            error = %e,
                            "Query execution failed"
                        );
                        // Update the error count metrics
                        counter!("surrealmcp.total_errors", 1);
                        counter!("surrealmcp.total_query_errors", 1);
                        // Update the query duration metric
                        histogram!("surrealmcp.query_duration_ms", duration.as_millis() as f64);
                        // Return error message
                        Err(McpError::internal_error(e.to_string(), None))
                    }
                }
            }
            None => {
                // Output debugging information
                warn!(
                    connection_id = %self.connection_id,
                    query_id,
                    query = %query_string,
                    "Query attempted without database connection"
                );
                // Update the query errors metric
                counter!("surrealmcp.total_errors", 1);
                counter!("surrealmcp.total_query_errors", 1);
                // Return error message
                Err(McpError::internal_error(
                    "Not connected to any SurrealDB endpoint. Use connect_endpoint first."
                        .to_string(),
                    None,
                ))
            }
        }
    }

    /// Create a new record in the specified table with the provided data.
    ///
    /// This function executes a SurrealDB CREATE statement to insert a new record
    /// into the specified table. The data is provided as a JSON value and will be
    /// used as the content for the new record. The table parameter can be either
    /// a table name or a specific record ID in the format "table:id".
    ///
    /// # Arguments
    /// * `table` - The table name or record ID where the new record will be created
    /// * `data` - The JSON data to be inserted as the record content
    #[tool(description = r#"
Create a new record in the specified table with the provided data.

This function executes a SurrealDB CREATE statement to insert a new record into the 
specified table. The data is provided as a JSON value and will be used as the content 
for the new record.

The table parameter can be either:
- A table name (SurrealDB will generate a unique ID)
- A specific record ID in the format 'table:id'

This is useful for creating users, articles, products, or any other entities in your database.

Examples:
- create('person', {"name": "John", "age": 30})
- create('person:john', {"name": "John", "age": 30})
- create('article', {"title": "SurrealDB Guide", "content": "..."})
"#)]
    pub async fn create(
        &self,
        params: Parameters<CreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let CreateParams { table, data } = params.0;
        debug!("Creating record in table: {table}");
        let query = format!("CREATE {table} CONTENT {data}");
        self.query(Parameters(QueryParams { query })).await
    }

    /// Execute a SurrealDB SELECT statement to retrieve records from the database.
    ///
    /// This function executes a SurrealDB SELECT statement to query records from
    /// the specified table or retrieve a specific record by ID. The thing parameter
    /// can be either a table name to select all records from that table, or a
    /// specific record ID in the format "table:id" to select a single record.
    /// The query results are returned as text, or an error occurs if the query
    /// execution fails.
    ///
    /// # Arguments
    /// * `thing` - The table name or record ID to select from
    #[tool(description = r#"
Execute a SurrealDB SELECT statement to retrieve records from the database.

This function executes a SurrealDB SELECT statement to query records from the specified 
table or retrieve a specific record by ID.

The thing parameter can be either:
- A table name to select all records from that table
- A specific record ID in the format 'table:id' to select a single record
- Complex SurrealQL syntax for filtered or related queries

You can also use complex SurrealQL syntax like 'person WHERE age > 25' or 'person:john.*' 
to get all related records.

Examples:
- select('person')
- select('person:john')
- select('person WHERE age > 25 ORDER BY name')
- select('person:john.*')
- select('person WHERE ->knows->person.age > 30')
"#)]
    pub async fn select(
        &self,
        params: Parameters<SelectParams>,
    ) -> Result<CallToolResult, McpError> {
        let SelectParams { thing } = params.0;
        debug!("Selecting records: {thing}");
        let query = format!("SELECT * FROM {thing}");
        self.query(Parameters(QueryParams { query })).await
    }

    /// Execute a SurrealDB UPDATE statement to modify records in the database.
    ///
    /// This function executes a SurrealDB UPDATE statement to modify the content
    /// of records in the database. The thing parameter can be either a table name
    /// to update all records in that table, or a specific record ID in the format
    /// "table:id" to update a single record. The update_mode parameter determines
    /// how the data is applied to the existing record.
    ///
    /// # Arguments
    /// * `thing` - The table name or record ID to update
    /// * `data` - The JSON data to apply to the record
    /// * `update_mode` - How to apply the data: "replace" (default), "merge", or "patch"
    #[tool(description = r#"
Execute a SurrealDB UPDATE statement to modify records in the database.

This function executes a SurrealDB UPDATE statement to modify the content of records 
in the database.

The thing parameter can be either:
- A table name to update all records in that table
- A specific record ID in the format 'table:id' to update a single record
- Complex queries for filtered updates

The update_mode parameter determines how the data is applied to the existing record:
- 'replace' (default): Completely replaces the record content
- 'merge': Combines new data with existing data
- 'patch': Applies JSON patch operations

Examples:
- update('person:john', {"age": 31}, None)
- update('person:john', {"city": "NYC"}, Some('merge'))
- update('person:john', [{"op": "replace", "path": "/age", "value": 31}], Some('patch'))
- update('person WHERE age < 18', {"status": "minor"}, None)
"#)]
    pub async fn update(
        &self,
        params: Parameters<UpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let UpdateParams {
            thing,
            data,
            update_mode,
        } = params.0;
        let mode = update_mode.as_deref().unwrap_or("replace");
        debug!("Updating records: {thing}");
        let query = match mode {
            "merge" => format!("UPDATE {thing} MERGE {data}"),
            "patch" => format!("UPDATE {thing} PATCH {data}"),
            _ => format!("UPDATE {thing} CONTENT {data}"), // replace is default
        };

        self.query(Parameters(QueryParams { query })).await
    }

    /// Execute a SurrealDB DELETE statement to remove records from the database.
    ///
    /// This function executes a SurrealDB DELETE statement to remove records from
    /// the specified table or delete a specific record by ID. The thing parameter
    /// can be either a table name to delete all records from that table, or a
    /// specific record ID in the format "table:id" to delete a single record.
    /// The query results are returned as text, or an error occurs if the query
    /// execution fails.
    ///
    /// # Arguments
    /// * `thing` - The table name or record ID to delete
    #[tool(description = r#"
Execute a SurrealDB DELETE statement to remove records from the database.

This function executes a SurrealDB DELETE statement to remove records from the 
specified table or delete a specific record by ID.

The thing parameter can be either:
- A table name to delete all records from that table
- A specific record ID in the format 'table:id' to delete a single record
- Complex queries for conditional deletion

The query results are returned as text, or an error occurs if the query execution fails.

Examples:
- delete('person:john')
- delete('person WHERE age < 18')
- delete('article WHERE published = false')
- delete('person')  # Deletes all records from person table
"#)]
    pub async fn delete(
        &self,
        params: Parameters<DeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let DeleteParams { thing } = params.0;
        debug!("Deleting records: {thing}");
        let query = format!("DELETE {thing}");
        self.query(Parameters(QueryParams { query })).await
    }

    /// Create a relationship between two records in the database.
    ///
    /// This function executes a SurrealDB RELATE statement to create a relationship
    /// between two records. The relationship is defined by the from_id, relationship_type,
    /// and to_id parameters. Optionally, you can provide content data to store on the
    /// relationship edge itself.
    ///
    /// # Arguments
    /// * `from_id` - The source record ID (e.g., "person:john")
    /// * `relationship_type` - The type of relationship (e.g., "wrote", "knows", "owns")
    /// * `to_id` - The target record ID (e.g., "article:surreal", "person:jane", "car:tesla")
    /// * `content` - Optional JSON data to store on the relationship edge
    #[tool(description = r#"
Create a relationship between two records in the database.

This function executes a SurrealDB RELATE statement to create a relationship between 
two records. The relationship is defined by the from_id, relationship_type, and to_id 
parameters.

Optionally, you can provide content data to store on the relationship edge itself. 
This is essential for graph operations and modeling complex relationships like social 
networks, content authorship, ownership, etc.

Examples:
- relate('person:john', 'wrote', 'article:surreal_guide', None)
- relate('person:john', 'knows', 'person:jane', {"since": "2020-01-01", "strength": "close"})
- relate('company:acme', 'employs', 'person:john', {"role": "developer", "start_date": "2023-01-01"})
- relate('user:alice', 'likes', 'post:123', {"timestamp": "2024-01-15T10:30:00Z"})
"#)]
    pub async fn relate(
        &self,
        params: Parameters<RelateParams>,
    ) -> Result<CallToolResult, McpError> {
        let RelateParams {
            from_id,
            relationship_type,
            to_id,
            content,
        } = params.0;
        debug!(
            "Creating relationship: {} -> {} -> {}",
            from_id, relationship_type, to_id
        );
        let query = match content {
            Some(content_data) => {
                format!("RELATE {from_id}->{relationship_type}->{to_id} CONTENT {content_data}")
            }
            None => format!("RELATE {from_id}->{relationship_type}->{to_id}"),
        };

        self.query(Parameters(QueryParams { query })).await
    }

    #[tool(description = "List SurrealDB Cloud organizations")]
    pub async fn list_cloud_organizations(&self) -> Result<CallToolResult, McpError> {
        debug!("Listing cloud organizations");
        let msg = "list_cloud_organizations not implemented".to_string();
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "List SurrealDB Cloud instances for a given organization")]
    pub async fn list_cloud_instances(
        &self,
        params: Parameters<CloudOrganizationParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudOrganizationParams { organization_id } = params.0;
        debug!("Listing cloud instances for organization: {organization_id}");
        let msg = "list_cloud_instances not implemented".to_string();
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "Pause SurrealDB Cloud instance")]
    pub async fn pause_cloud_instance(
        &self,
        params: Parameters<CloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudInstanceParams { instance_id } = params.0;
        debug!("Pausing cloud instance: {instance_id}");
        let msg = "pause_cloud_instance not implemented".to_string();
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "Resume SurrealDB Cloud instance")]
    pub async fn resume_cloud_instance(
        &self,
        params: Parameters<CloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudInstanceParams { instance_id } = params.0;
        debug!("Resuming cloud instance: {instance_id}");
        let msg = "resume_cloud_instance not implemented".to_string();
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "Resume SurrealDB Cloud instance")]
    pub async fn get_cloud_instance_status(
        &self,
        params: Parameters<CloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudInstanceParams { instance_id } = params.0;
        debug!("Getting status for cloud instance: {instance_id}");
        let msg = "get_cloud_instance_status not implemented".to_string();
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "Resume SurrealDB Cloud instance")]
    pub async fn get_cloud_instance_metrics(
        &self,
        params: Parameters<CloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudInstanceParams { instance_id } = params.0;
        debug!("Getting metrics for cloud instance: {instance_id}");
        let msg = "get_cloud_instance_metrics not implemented".to_string();
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "Create SurrealDB Cloud instance")]
    pub async fn create_cloud_instance(
        &self,
        params: Parameters<CreateCloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CreateCloudInstanceParams {
            name,
            organization_id,
        } = params.0;
        debug!("Creating cloud instance: {name} in organization: {organization_id}");
        let msg = "create_cloud_instance not implemented".to_string();
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    /// Connect to a different SurrealDB endpoint.
    ///
    /// This function allows you to dynamically connect to a different SurrealDB
    /// endpoint during your session. The endpoint can be any supported SurrealDB
    /// engine type (memory, rocksdb, surrealkv, tikv, http, https, ws, wss).
    ///
    /// # Arguments
    /// * `endpoint` - The SurrealDB endpoint URL (e.g., "memory", "file:/path/to/db", "ws://localhost:8000")
    /// * `namespace` - The namespace to use (optional, defaults to "test")
    /// * `database` - The database to use (optional, defaults to "test")
    /// * `username` - Username for authentication (optional, only needed for remote connections)
    /// * `password` - Password for authentication (optional, only needed for remote connections)
    #[tool(description = r#"
Connect to a different SurrealDB endpoint.

This function allows you to dynamically connect to a different SurrealDB endpoint 
during your session. The endpoint can be any supported SurrealDB engine type including 
memory (for testing), file-based storage, distributed storage, or remote connections.

Each client connection is completely isolated, so you can switch between different 
databases as needed.

Examples:
- connect_endpoint('memory', None, None, None, None)  # For testing
- connect_endpoint('file:/tmp/mydb', Some('myapp'), Some('production'), None, None)  # Local file storage
- connect_endpoint('ws://localhost:8000', Some('myapp'), Some('production'), Some('root'), Some('password'))  # Remote connection
- connect_endpoint('rocksdb:/data/mydb', Some('analytics'), Some('events'), None, None)  # High-performance local storage
"#)]
    pub async fn connect_endpoint(
        &self,
        params: Parameters<ConnectParams>,
    ) -> Result<CallToolResult, McpError> {
        let ConnectParams {
            endpoint,
            namespace,
            database,
            username,
            password,
        } = params.0;
        let start_time = Instant::now();
        // Output debugging information
        info!(
            connection_id = %self.connection_id,
            endpoint = %endpoint,
            namespace = namespace.as_deref(),
            database = database.as_deref(),
            has_username = username.is_some(),
            "Attempting to connect to SurrealDB endpoint"
        );
        // Check if endpoint is restricted by startup configuration
        if let Some(configured_endpoint) = &self.endpoint {
            if endpoint != *configured_endpoint {
                // Output debugging information
                warn!(
                    connection_id = %self.connection_id,
                    requested_endpoint = %endpoint,
                    configured_endpoint = %configured_endpoint,
                    "Connection rejected: endpoint not allowed by server configuration"
                );
                // Return error message
                return Err(McpError::internal_error(
                    format!(
                        "Cannot connect to endpoint '{endpoint}'. Server is configured to only use endpoint '{configured_endpoint}'"
                    ),
                    None,
                ));
            }
        }
        // Check if namespace is restricted by startup configuration
        if let Some(configured_namespace) = &self.namespace {
            if let Some(namespace) = &namespace {
                if namespace != configured_namespace {
                    // Output debugging information
                    warn!(
                        connection_id = %self.connection_id,
                        requested_namespace = %namespace,
                        configured_namespace = %configured_namespace,
                        "Connection rejected: namespace not allowed by server configuration"
                    );
                    // Return error message
                    return Err(McpError::internal_error(
                        format!(
                            "Cannot use namespace '{namespace}'. Server is configured to only use namespace '{configured_namespace}'"
                        ),
                        None,
                    ));
                }
            }
        }
        // Check if database is restricted by startup configuration
        if let Some(configured_database) = &self.database {
            if let Some(database) = &database {
                if database != configured_database {
                    // Output debugging information
                    warn!(
                        connection_id = %self.connection_id,
                        requested_database = %database,
                        configured_database = %configured_database,
                        "Connection rejected: database not allowed by server configuration"
                    );
                    // Return error message
                    return Err(McpError::internal_error(
                        format!(
                            "Cannot use database '{database}'. Server is configured to only use database '{configured_database}'"
                        ),
                        None,
                    ));
                }
            }
        }
        // Get the namespace to use for the connection
        let ns = namespace.or_else(|| self.namespace.clone());
        // Get the database to use for the connection
        let db = database.or_else(|| self.database.clone());
        // Get the username to use for authentication
        let user = username.or_else(|| self.user.clone());
        // Get the password to use for authentication
        let pass = password.or_else(|| self.pass.clone());
        // Create a new SurrealDB connection
        match db::create_client_connection(
            &endpoint,
            user.as_deref(),
            pass.as_deref(),
            ns.as_deref(),
            db.as_deref(),
        )
        .await
        {
            Ok(instance) => {
                let duration = start_time.elapsed();
                // Update the service's database connection
                let mut db_guard = self.db.lock().await;
                *db_guard = Some(instance);
                // Output debugging information
                info!(
                    connection_id = %self.connection_id,
                    endpoint = %endpoint,
                    namespace = ns.as_deref(),
                    database = db.as_deref(),
                    duration_ms = duration.as_millis(),
                    "Successfully connected to SurrealDB endpoint"
                );
                // Return success message
                let msg = format!("Successfully connected to endpoint '{endpoint}'");
                Ok(CallToolResult::success(vec![Content::text(msg)]))
            }
            Err(e) => {
                let duration = start_time.elapsed();
                // Output debugging information
                error!(
                    connection_id = %self.connection_id,
                    endpoint = %endpoint,
                    namespace = ns.as_deref(),
                    database = db.as_deref(),
                    duration_ms = duration.as_millis(),
                    error = %e,
                    "Failed to connect to SurrealDB endpoint"
                );
                // Increment error metrics
                counter!("surrealmcp.total_errors", 1);
                // Return error message
                Err(McpError::internal_error(
                    format!("Failed to connect to endpoint '{endpoint}': {e}"),
                    None,
                ))
            }
        }
    }

    /// Change the namespace on the currently connected endpoint.
    ///
    /// This function allows you to switch to a different namespace on the currently
    /// connected SurrealDB endpoint. The namespace change will apply to all subsequent
    /// queries until you change it again or reconnect to a different endpoint.
    ///
    /// # Arguments
    /// * `namespace` - The namespace to switch to
    #[tool(description = r#"
Change the namespace on the currently connected endpoint.

This function allows you to switch to a different namespace on the currently connected 
SurrealDB endpoint. The namespace change will apply to all subsequent queries until 
you change it again or reconnect to a different endpoint.

This is useful when you want to:
- Organize data into different logical groups
- Switch between development, staging, and production environments
- Work with multiple applications using the same SurrealDB instance

Examples:
- use_namespace('development')
- use_namespace('production')
- use_namespace('analytics')
"#)]
    pub async fn use_namespace(
        &self,
        params: Parameters<UseNamespaceParams>,
    ) -> Result<CallToolResult, McpError> {
        let UseNamespaceParams { namespace } = params.0;
        let start_time = Instant::now();
        // Output debugging information
        info!(
            connection_id = %self.connection_id,
            namespace = %namespace,
            "Attempting to change namespace"
        );
        // Check if namespace is restricted by startup configuration
        if let Some(configured_namespace) = &self.namespace {
            if namespace != *configured_namespace {
                // Output debugging information
                warn!(
                    connection_id = %self.connection_id,
                    requested_namespace = %namespace,
                    configured_namespace = %configured_namespace,
                    "Namespace change rejected: namespace not allowed by server configuration"
                );
                // Return error message
                return Err(McpError::internal_error(
                    format!(
                        "Cannot use namespace '{namespace}'. Server is configured to only use namespace '{configured_namespace}'"
                    ),
                    None,
                ));
            }
        }
        // Lock the database connection
        let db_guard = self.db.lock().await;
        // Match the database connection
        match &*db_guard {
            Some(db) => {
                // Use the specified namespace
                match db.use_ns(&namespace).await {
                    Ok(_) => {
                        let duration = start_time.elapsed();
                        // Output debugging information
                        info!(
                            connection_id = %self.connection_id,
                            namespace = %namespace,
                            duration_ms = duration.as_millis(),
                            "Successfully changed namespace"
                        );
                        // Return success message
                        let msg = format!("Successfully switched to namespace '{namespace}'");
                        Ok(CallToolResult::success(vec![Content::text(msg)]))
                    }
                    Err(e) => {
                        let duration = start_time.elapsed();
                        // Output debugging information
                        error!(
                            connection_id = %self.connection_id,
                            namespace = %namespace,
                            duration_ms = duration.as_millis(),
                            error = %e,
                            "Failed to change namespace"
                        );
                        // Increment error metrics
                        counter!("surrealmcp.total_errors", 1);
                        // Return error message
                        Err(McpError::internal_error(
                            format!("Failed to change namespace to '{namespace}': {e}"),
                            None,
                        ))
                    }
                }
            }
            None => {
                // Output debugging information
                warn!(
                    connection_id = %self.connection_id,
                    namespace = %namespace,
                    "Namespace change attempted without database connection"
                );
                // Return error message
                Err(McpError::internal_error(
                    "Not connected to any SurrealDB endpoint. Use connect_endpoint first."
                        .to_string(),
                    None,
                ))
            }
        }
    }

    /// Change the database on the currently connected endpoint.
    ///
    /// This function allows you to switch to a different database on the currently
    /// connected SurrealDB endpoint. The database change will apply to all subsequent
    /// queries until you change it again or reconnect to a different endpoint.
    ///
    /// # Arguments
    /// * `database` - The database to switch to
    #[tool(description = r#"
Change the database on the currently connected endpoint.

This function allows you to switch to a different database on the currently connected 
SurrealDB endpoint. The database change will apply to all subsequent queries until 
you change it again or reconnect to a different endpoint.

This is useful when you want to:
- Switch between different databases within the same namespace
- Organize data into different logical groups
- Work with multiple applications using the same SurrealDB instance

Examples:
- use_database('users')
- use_database('analytics')
- use_database('events')
"#)]
    pub async fn use_database(
        &self,
        params: Parameters<UseDatabaseParams>,
    ) -> Result<CallToolResult, McpError> {
        let UseDatabaseParams { database } = params.0;
        let start_time = Instant::now();
        // Output debugging information
        info!(
            connection_id = %self.connection_id,
            database = %database,
            "Attempting to change database"
        );
        // Check if database is restricted by startup configuration
        if let Some(configured_database) = &self.database {
            if database != *configured_database {
                // Output debugging information
                warn!(
                    connection_id = %self.connection_id,
                    requested_database = %database,
                    configured_database = %configured_database,
                    "Database change rejected: database not allowed by server configuration"
                );
                // Return error message
                return Err(McpError::internal_error(
                    format!(
                        "Cannot use database '{database}'. Server is configured to only use database '{configured_database}'"
                    ),
                    None,
                ));
            }
        }
        // Lock the database connection
        let db_guard = self.db.lock().await;
        // Match the database connection
        match &*db_guard {
            Some(db) => {
                // Use the specified database
                match db.use_db(&database).await {
                    Ok(_) => {
                        let duration = start_time.elapsed();
                        // Output debugging information
                        info!(
                            connection_id = %self.connection_id,
                            database = %database,
                            duration_ms = duration.as_millis(),
                            "Successfully changed database"
                        );
                        // Return success message
                        let msg = format!("Successfully switched to database '{database}'");
                        Ok(CallToolResult::success(vec![Content::text(msg)]))
                    }
                    Err(e) => {
                        let duration = start_time.elapsed();
                        // Output debugging information
                        error!(
                            connection_id = %self.connection_id,
                            database = %database,
                            duration_ms = duration.as_millis(),
                            error = %e,
                            "Failed to change database"
                        );
                        // Increment error metrics
                        counter!("surrealmcp.total_errors", 1);
                        // Return error message
                        Err(McpError::internal_error(
                            format!("Failed to change database to '{database}': {e}"),
                            None,
                        ))
                    }
                }
            }
            None => {
                // Output debugging information
                warn!(
                    connection_id = %self.connection_id,
                    database = %database,
                    "Database change attempted without database connection"
                );
                // Return error message
                Err(McpError::internal_error(
                    "Not connected to any SurrealDB endpoint. Use connect_endpoint first."
                        .to_string(),
                    None,
                ))
            }
        }
    }

    /// Disconnect from the current SurrealDB endpoint.
    ///
    /// This function disconnects from the currently connected SurrealDB endpoint.
    /// After disconnecting, you'll need to use connect_endpoint again to establish
    /// a new connection before you can execute queries.
    #[tool(description = r#"
Disconnect from the current SurrealDB endpoint.

This function disconnects from the currently connected SurrealDB endpoint.
After disconnecting, you'll need to use connect_endpoint again to establish
a new connection before you can execute queries.

This is useful when you want to:
- Switch to a different database
- Clean up resources
- Ensure no active connections remain
"#)]
    pub async fn disconnect_endpoint(&self) -> Result<CallToolResult, McpError> {
        // Output debugging information
        info!(
            connection_id = %self.connection_id,
            "Disconnecting from SurrealDB endpoint"
        );
        // Lock the database connection
        let mut db_guard = self.db.lock().await;
        // Set the database connection to None
        *db_guard = None;
        // Output debugging information
        info!(
            connection_id = %self.connection_id,
            "Successfully disconnected from SurrealDB endpoint"
        );
        // Return success message
        Ok(CallToolResult::success(vec![Content::text(
            "Successfully disconnected from SurrealDB endpoint".to_string(),
        )]))
    }

    /// Initialize the database connection using startup configuration.
    ///
    /// This method attempts to connect to the database using the configuration
    /// provided at startup. If no endpoint is configured, this method does nothing.
    /// If an endpoint is configured, it will connect using the configured settings.
    pub async fn initialize_connection(&self) -> Result<(), anyhow::Error> {
        if let Some(endpoint) = &self.endpoint {
            // Output debugging information
            info!(
                connection_id = %self.connection_id,
                endpoint = %endpoint,
                namespace = self.namespace.as_deref(),
                database = self.database.as_deref(),
                "Initializing database connection with startup configuration"
            );
            // Get the configured endpoint details
            let user = self.user.as_deref();
            let pass = self.pass.as_deref();
            let ns = self.namespace.as_deref();
            let db = self.database.as_deref();
            // Create a new SurrealDB connection
            match db::create_client_connection(endpoint, user, pass, ns, db).await {
                Ok(instance) => {
                    // Update the service's database connection
                    let mut db_guard = self.db.lock().await;
                    *db_guard = Some(instance);
                    // Output debugging information
                    info!(
                        connection_id = %self.connection_id,
                        endpoint = %endpoint,
                        "Successfully initialized database connection"
                    );
                }
                Err(e) => {
                    // Output debugging information
                    error!(
                        connection_id = %self.connection_id,
                        endpoint = %endpoint,
                        error = %e,
                        "Failed to initialize database connection"
                    );
                    // Return error message
                    return Err(e);
                }
            }
        } else {
            debug!(
                connection_id = %self.connection_id,
                "No endpoint configured for startup connection"
            );
        }
        // All ok
        Ok(())
    }
}

#[tool_handler]
impl ServerHandler for SurrealService {
    fn get_info(&self) -> ServerInfo {
        debug!("Getting server info");
        ServerInfo {
            capabilities: ServerCapabilities::builder()
                .enable_tools()
                .enable_resources()
                .enable_prompts()
                .build(),
            instructions: Some(include_str!("../../instructions.md").to_string()),
            ..Default::default()
        }
    }

    async fn initialize(
        &self,
        _req: rmcp::model::InitializeRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::InitializeResult, McpError> {
        debug!("Initializing MCP server");
        // Initialize the connection using startup configuration
        if let Err(e) = self.initialize_connection().await {
            error!(
                connection_id = %self.connection_id,
                error = %e,
                "Failed to initialize database connection during MCP initialization"
            );
        }
        Ok(self.get_info())
    }
}
