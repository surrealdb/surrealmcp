use anyhow::{Result, anyhow};
use clap::{Parser, Subcommand};
use rmcp::{
    Error as McpError, RoleServer, ServerHandler,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool,
};
use std::env;
use std::path::Path;
use std::sync::Arc;
use surrealdb::opt::auth::Root;
use surrealdb::{Surreal, engine::any, engine::any::Any};
use tokio::fs;
use tokio::net::{TcpListener, UnixListener};
use tokio::sync::Mutex;

#[derive(Parser)]
#[command(name = "surrealmcp")]
#[command(about = "SurrealDB MCP Server")]
#[command(version)]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Start the MCP server
    Start {
        /// The SurrealDB endpoint URL to connect to
        #[arg(short, long, env = "SURREALDB_URL")]
        endpoint: Option<String>,
        /// The SurrealDB namespace to use
        #[arg(long, env = "SURREALDB_NS")]
        ns: Option<String>,
        /// The SurrealDB database to use
        #[arg(long, env = "SURREALDB_DB")]
        db: Option<String>,
        /// The SurrealDB username to use
        #[arg(short, long, env = "SURREALDB_USER")]
        user: Option<String>,
        /// The SurrealDB password to use
        #[arg(short, long, env = "SURREALDB_PASS")]
        pass: Option<String>,
        /// The MCP server listen address
        #[arg(
            short,
            long,
            env = "SURREAL_MCP_LISTEN",
            default_value = "0.0.0.0:8080"
        )]
        listen: String,
    },
}

/// Create a new SurrealDB connection for a client
async fn create_client_connection(
    url: &str,
    username: Option<&str>,
    password: Option<&str>,
    namespace: Option<&str>,
    database: Option<&str>,
) -> Result<Surreal<Any>, anyhow::Error> {
    // Connect to SurrealDB using the Any engine
    let instance = any::connect(url)
        .await
        .map_err(|e| anyhow!(e.to_string()))?;
    // Attempt to authenticate if remote
    if let (Some(username), Some(password)) = (username, password) {
        instance
            .signin(Root { username, password })
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
    }
    // Set namespace if provided
    if let Some(ns) = namespace {
        instance
            .use_ns(ns)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
    }
    // Set database if provided
    if let Some(db) = database {
        instance
            .use_db(db)
            .await
            .map_err(|e| anyhow!(e.to_string()))?;
    }
    // Return the instance
    Ok(instance)
}

#[derive(Clone)]
struct SurrealService {
    /// The SurrealDB client instance to use for database operations
    db: Arc<Mutex<Option<Surreal<Any>>>>,
    /// The configured SurrealDB endpoint URL (optionally set at server startup)
    endpoint: Option<String>,
    /// The configured SurrealDB namespace (optionally set at server startup)
    namespace: Option<String>,
    /// The configured SurrealDB database (optionally set at server startup)
    database: Option<String>,
    /// The configured SurrealDB username (optionally set at server startup)
    user: Option<String>,
    /// The configured SurrealDB password (optionally set at server startup)
    pass: Option<String>,
}

#[tool(tool_box)]
impl SurrealService {
    /// Create a new SurrealService instance with the provided database connection.
    ///
    /// This function initializes a new SurrealService instance that can be used
    /// to interact with a SurrealDB database. The database connection is provided
    /// as a SurrealDB client instance, which is used to execute queries and
    /// perform database operations.
    ///
    /// # Arguments
    /// * `db` - The SurrealDB client instance to use for database operations
    #[allow(dead_code)]
    pub fn new() -> Self {
        Self {
            db: Arc::new(Mutex::new(None)),
            endpoint: None,
            namespace: None,
            database: None,
            user: None,
            pass: None,
        }
    }

    /// Create a new SurrealService instance with startup configuration.
    ///
    /// This function initializes a new SurrealService instance with predefined
    /// configuration that restricts what endpoints, namespaces, and databases
    /// can be used during the session.
    ///
    /// # Arguments
    /// * `endpoint` - The SurrealDB endpoint URL (optional)
    /// * `namespace` - The namespace to use (optional)
    /// * `database` - The database to use (optional)
    /// * `user` - Username for authentication (optional)
    /// * `pass` - Password for authentication (optional)
    pub fn with_config(
        endpoint: Option<String>,
        namespace: Option<String>,
        database: Option<String>,
        user: Option<String>,
        pass: Option<String>,
    ) -> Self {
        Self {
            db: Arc::new(Mutex::new(None)),
            endpoint,
            namespace,
            database,
            user,
            pass,
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
    async fn query(&self, #[tool(param)] query_string: String) -> Result<CallToolResult, McpError> {
        let db_guard = self.db.lock().await;

        match &*db_guard {
            Some(db) => match db.query(query_string).await {
                Ok(res) => {
                    let text = format!("{res:?}");
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Err(McpError::internal_error(e.to_string(), None)),
            },
            None => Err(McpError::internal_error(
                "Not connected to any SurrealDB endpoint. Use connect_endpoint first.".to_string(),
                None,
            )),
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
    async fn create(
        &self,
        #[tool(param)]
        #[schemars(description = r#"
The table name or record ID where the new record will be created.

Can be a simple table name like 'person' or a specific record ID like 'person:john'.
If you provide a table name, SurrealDB will generate a unique ID.
If you provide a specific record ID, that exact ID will be used.

Examples: 'person', 'person:john', 'article:surreal_guide'
"#)]
        table: String,
        #[tool(param)]
        #[schemars(description = r#"
The JSON data to be inserted as the record content.

This can be any valid JSON object with nested objects, arrays, strings, numbers, 
booleans, etc.

Example:
{
  "name": "John",
  "age": 30,
  "tags": ["developer", "rust"],
  "profile": {
    "city": "NYC",
    "website": "https://example.com"
  }
}
"#)]
        data: serde_json::Value,
    ) -> Result<CallToolResult, McpError> {
        let query = format!("CREATE {table} CONTENT {data}");
        let db_guard = self.db.lock().await;

        match &*db_guard {
            Some(db) => match db.query(query).await {
                Ok(res) => {
                    let text = format!("{res:?}");
                    Ok(CallToolResult::success(vec![Content::text(text)]))
                }
                Err(e) => Err(McpError::internal_error(e.to_string(), None)),
            },
            None => Err(McpError::internal_error(
                "Not connected to any SurrealDB endpoint. Use connect_endpoint first.".to_string(),
                None,
            )),
        }
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
    async fn select(
        &self,
        #[tool(param)]
        #[schemars(description = r#"
The table name, record ID, or complex query to select from.

Examples:
- 'person' (all records from person table)
- 'person:john' (specific record with ID 'john')
- 'person WHERE age > 25' (filtered records)
- 'person:john.*' (all related records)
- 'person WHERE ->knows->person.age > 30' (graph query)
"#)]
        thing: String,
    ) -> Result<CallToolResult, McpError> {
        let query = format!("SELECT {thing}");
        self.query(query).await
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
    async fn update(
        &self,
        #[tool(param)]
        #[schemars(description = r#"
The table name, record ID, or complex query to update.

Examples:
- 'person:john' (specific record)
- 'person WHERE age < 18' (filtered records)
- 'person' (all records in table)
- 'article WHERE published = false' (conditional update)
"#)]
        thing: String,
        #[tool(param)]
        #[schemars(description = r#"
The JSON data to apply to the record.

For 'replace' mode: This should be the complete record content.
For 'merge' mode: This can be partial data that will be combined with existing data.
For 'patch' mode: This should be a JSON patch array with operations.

Examples:
Replace: {"name": "John", "age": 31, "city": "NYC"}
Merge: {"city": "NYC", "phone": "123-456-7890"}
Patch: [{"op": "replace", "path": "/age", "value": 31}]
"#)]
        data: serde_json::Value,
        #[tool(param)]
        #[schemars(description = r#"
Update mode for applying data to existing records.

- 'replace' (default): Replaces entire record content
- 'merge': Combines new data with existing data
- 'patch': Applies JSON patch operations

Choose based on your needs:
- Use 'replace' when you want to completely overwrite the record
- Use 'merge' when you want to add/update specific fields
- Use 'patch' for precise field-level changes
"#)]
        update_mode: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        let mode = update_mode.as_deref().unwrap_or("replace");
        let query = match mode {
            "merge" => format!("UPDATE {thing} MERGE {data}"),
            "patch" => format!("UPDATE {thing} PATCH {data}"),
            _ => format!("UPDATE {thing} CONTENT {data}"), // replace is default
        };

        self.query(query).await
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
    async fn delete(
        &self,
        #[tool(param)]
        #[schemars(description = r#"
The table name, record ID, or complex query to delete.

Examples:
- 'person:john' (specific record)
- 'person WHERE age < 18' (filtered records)
- 'article WHERE published = false' (conditional deletion)
- 'person' (all records in table - use with caution!)
"#)]
        thing: String,
    ) -> Result<CallToolResult, McpError> {
        let query = format!("DELETE {thing}");
        self.query(query).await
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
    async fn relate(
        &self,
        #[tool(param)]
        #[schemars(description = r#"
The source record ID in 'table:id' format.

This is the record that the relationship starts from.

Examples:
- 'person:john'
- 'article:surreal_guide'
- 'company:acme'
- 'user:alice'
"#)]
        from_id: String,
        #[tool(param)]
        #[schemars(description = r#"
The type of relationship that describes the connection between records.

Examples:
- 'wrote', 'authored', 'created' (content relationships)
- 'knows', 'follows', 'friends' (social relationships)
- 'owns', 'belongs_to', 'part_of' (ownership relationships)
- 'works_for', 'manages', 'employs' (work relationships)
- 'likes', 'dislikes', 'rates' (preference relationships)
"#)]
        relationship_type: String,
        #[tool(param)]
        #[schemars(description = r#"
The target record ID in 'table:id' format.

This is the record that the relationship points to.

Examples:
- 'article:surreal_guide'
- 'person:jane'
- 'company:acme'
- 'post:123'
"#)]
        to_id: String,
        #[tool(param)]
        #[schemars(description = r#"
Optional JSON data to store on the relationship edge.

This can include metadata like dates, weights, descriptions, ratings, etc.

Examples:
- {"since": "2020-01-01", "strength": "close"}
- {"date": "2024-01-15", "word_count": 1500}
- {"role": "developer", "start_date": "2023-01-01"}
- {"rating": 5, "comment": "Great article!"}
"#)]
        content: Option<serde_json::Value>,
    ) -> Result<CallToolResult, McpError> {
        let query = match content {
            Some(content_data) => {
                format!("RELATE {from_id}->{relationship_type}->{to_id} CONTENT {content_data}")
            }
            None => format!("RELATE {from_id}->{relationship_type}->{to_id}"),
        };

        self.query(query).await
    }

    #[tool(description = "List Surreal Cloud instances (placeholder)")]
    async fn list_cloud_instances(&self) -> Result<CallToolResult, McpError> {
        Ok(CallToolResult::success(vec![Content::text(
            "cloud list not implemented".to_string(),
        )]))
    }

    #[tool(description = "Pause Surreal Cloud instance (placeholder)")]
    async fn pause_cloud_instance(
        &self,
        #[tool(param)] instance_id: String,
    ) -> Result<CallToolResult, McpError> {
        let msg = format!("pause {instance_id} not implemented");
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "Resume Surreal Cloud instance (placeholder)")]
    async fn resume_cloud_instance(
        &self,
        #[tool(param)] instance_id: String,
    ) -> Result<CallToolResult, McpError> {
        let msg = format!("resume {instance_id} not implemented");
        Ok(CallToolResult::success(vec![Content::text(msg)]))
    }

    #[tool(description = "Create Surreal Cloud instance (placeholder)")]
    async fn create_cloud_instance(
        &self,
        #[tool(param)] name: String,
    ) -> Result<CallToolResult, McpError> {
        let msg = format!("create instance '{name}' not implemented");
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
    async fn connect_endpoint(
        &self,
        #[tool(param)]
        #[schemars(description = r#"
The SurrealDB endpoint URL.

Supported formats:
- 'memory' (in-memory, no persistence)
- 'file:/path/to/db' (local file storage)
- 'rocksdb:/path/to/db' (high-performance local storage)
- 'tikv://localhost:2379' (distributed storage)
- 'ws://localhost:8000' (WebSocket remote)
- 'http://localhost:8000' (HTTP remote)
"#)]
        endpoint: String,
        #[tool(param)]
        #[schemars(description = r#"
The namespace to use for organizing data.

Namespaces provide logical separation of data.

Examples: 'myapp', 'production', 'development', 'test'
Defaults to 'test' if not specified
"#)]
        namespace: Option<String>,
        #[tool(param)]
        #[schemars(description = r#"
The database name within the namespace.

Databases provide further logical separation.

Examples: 'users', 'products', 'analytics', 'main'
Defaults to 'test' if not specified
"#)]
        database: Option<String>,
        #[tool(param)]
        #[schemars(description = r#"
Username for authentication.

Only required for remote connections (ws://, wss://, http://, https://).
For local engines (memory, file, rocksdb, tikv), authentication is not needed.

Examples: 'root', 'admin', 'user'
"#)]
        username: Option<String>,
        #[tool(param)]
        #[schemars(description = r#"
Password for authentication.

Only required for remote connections (ws://, wss://, http://, https://).
For local engines (memory, file, rocksdb, tikv), authentication is not needed.

Examples: 'password', 'secret123', 'admin_pass'
"#)]
        password: Option<String>,
    ) -> Result<CallToolResult, McpError> {
        // Check if endpoint is restricted by startup configuration
        if let Some(configured_endpoint) = &self.endpoint {
            if endpoint != *configured_endpoint {
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
        match create_client_connection(
            &endpoint,
            user.as_deref(),
            pass.as_deref(),
            ns.as_deref(),
            db.as_deref(),
        )
        .await
        {
            Ok(instance) => {
                // Update the service's database connection
                let mut db_guard = self.db.lock().await;
                *db_guard = Some(instance);

                let msg = format!("Successfully connected to endpoint '{endpoint}'");
                Ok(CallToolResult::success(vec![Content::text(msg)]))
            }
            Err(e) => Err(McpError::internal_error(
                format!("Failed to connect to endpoint '{endpoint}': {e}"),
                None,
            )),
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
    async fn disconnect_endpoint(&self) -> Result<CallToolResult, McpError> {
        let mut db_guard = self.db.lock().await;
        *db_guard = None;

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
            let user = self.user.as_deref();
            let pass = self.pass.as_deref();
            let ns = self.namespace.as_deref();
            let db = self.database.as_deref();

            let instance = create_client_connection(endpoint, user, pass, ns, db).await?;
            let mut db_guard = self.db.lock().await;
            *db_guard = Some(instance);
        }
        Ok(())
    }
}

#[tool(tool_box)]
impl ServerHandler for SurrealService {
    fn get_info(&self) -> ServerInfo {
        ServerInfo {
            capabilities: ServerCapabilities::builder().enable_tools().build(),
            instructions: Some(include_str!("../instructions.md").to_string()),
            ..Default::default()
        }
    }

    async fn initialize(
        &self,
        _req: rmcp::model::InitializeRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::InitializeResult, McpError> {
        Ok(self.get_info())
    }
}

#[tokio::main]
async fn main() -> Result<()> {
    // Parse command line arguments
    let cli = Cli::parse();
    // Run the specified command
    match cli.command {
        Commands::Start {
            endpoint,
            ns,
            db,
            user,
            pass,
            listen,
        } => {
            // Check if we should connect to a unix socket
            let socket_path = env::var("MCP_SOCKET_PATH");
            // Handle both TCP and Unix socket connections
            if let Ok(socket_path) = socket_path {
                // Unix socket mode
                let socket_path = Path::new(&socket_path);
                // Remove existing socket file if it exists
                if socket_path.exists() {
                    fs::remove_file(socket_path).await?;
                }
                // Create a Unix domain socket listener at the specified path
                let listener = UnixListener::bind(socket_path)?;
                // Log that the server is listening on the Unix socket
                println!(
                    "MCP server listening on Unix socket: {}",
                    socket_path.display()
                );
                // Main server loop for Unix socket connections
                loop {
                    // Accept incoming connections from the Unix socket
                    let (stream, _) = listener.accept().await?;
                    // Clone configuration values for this connection
                    let endpoint = endpoint.clone();
                    let namespace = ns.clone();
                    let database = db.clone();
                    let user = user.clone();
                    let pass = pass.clone();
                    // Spawn a new async task to handle this client connection
                    tokio::spawn(async move {
                        let service =
                            SurrealService::with_config(endpoint, namespace, database, user, pass);
                        // Initialize the connection using startup configuration only if endpoint is specified
                        if let Err(e) = service.initialize_connection().await {
                            eprintln!("Failed to initialize database connection: {e}");
                        }
                        // Create an MCP server instance for this connection
                        if let Ok(server) = rmcp::serve_server(service, stream).await {
                            // Wait for the server to complete its work
                            let _ = server.waiting().await;
                        }
                    });
                }
            } else {
                // Create a TCP listener bound to the specified address and port
                let listener = TcpListener::bind(&listen).await?;
                // Log that the server is listening on TCP
                println!("MCP server listening on TCP: {listen}");
                // Main server loop for TCP connections
                loop {
                    // Accept incoming TCP connections
                    let (stream, _) = listener.accept().await?;
                    // Clone configuration values for this connection
                    let endpoint = endpoint.clone();
                    let namespace = ns.clone();
                    let database = db.clone();
                    let user = user.clone();
                    let pass = pass.clone();
                    // Spawn a new async task to handle this client connection
                    tokio::spawn(async move {
                        let service =
                            SurrealService::with_config(endpoint, namespace, database, user, pass);
                        // Initialize the connection using startup configuration only if endpoint is specified
                        if let Err(e) = service.initialize_connection().await {
                            eprintln!("Failed to initialize database connection: {e}");
                        }
                        // Create an MCP server instance for this connection
                        if let Ok(server) = rmcp::serve_server(service, stream).await {
                            // Wait for the server to complete its work
                            let _ = server.waiting().await;
                        }
                    });
                }
            }
        }
    }
}
