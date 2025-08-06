use anyhow::Result;
use http::request::Parts;
use metrics::counter;
use rmcp::{
    ErrorData as McpError, RoleServer, ServerHandler,
    handler::server::router::tool::ToolRouter,
    handler::server::tool::Parameters,
    model::{CallToolResult, Content, ServerCapabilities, ServerInfo},
    service::RequestContext,
    tool, tool_handler, tool_router,
};
use serde::Deserialize;
use std::collections::HashMap;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::Instant;
use surrealdb::{Surreal, Value, engine::any::Any};
use tokio::sync::Mutex;
use tracing::{debug, error, info, trace, warn};

use crate::cloud::Client;
use crate::db;
use crate::engine;
use crate::prompts;
use crate::utils::{convert_json_to_surreal, parse_target, parse_targets};

// Global metrics
static QUERY_COUNTER: AtomicU64 = AtomicU64::new(0);

#[derive(Deserialize, schemars::JsonSchema)]
pub struct QueryParams {
    #[schemars(description = "The SurrealQL query string")]
    pub query: String,
    #[schemars(description = "Optional parameters to bind to the query")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct SelectParams {
    #[schemars(description = "Array of table names or record IDs to select from.")]
    pub targets: Vec<String>,
    #[schemars(description = "Optional WHERE clause to filter records.")]
    pub where_clause: Option<String>,
    #[schemars(description = "Optional SPLIT ON clause to split records on specific fields.")]
    pub split_clause: Option<String>,
    #[schemars(description = "Optional GROUP BY clause to group records by specific fields.")]
    pub group_clause: Option<String>,
    #[schemars(description = "Optional ORDER BY clause to sort records by specific fields.")]
    pub order_clause: Option<String>,
    #[schemars(description = "Optional LIMIT clause to limit the number of results.")]
    pub limit_clause: Option<String>,
    #[schemars(description = "Optional START clause to specify the pagination start position.")]
    pub start_clause: Option<String>,
    #[schemars(description = "Optional parameters to bind to the query.")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct InsertParams {
    #[schemars(description = "The table name into which we will insert data.")]
    pub target: String,
    #[schemars(description = "Whether to ignore duplicate records (INSERT IGNORE).")]
    pub ignore: Option<bool>,
    #[schemars(description = "Whether this is a relation table insert (INSERT RELATION).")]
    pub relation: Option<bool>,
    #[schemars(description = "Array of JSON objects to be inserted as the record content.")]
    pub values: Vec<serde_json::Map<String, serde_json::Value>>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct CreateParams {
    #[schemars(description = "A table name or record ID to create.")]
    pub target: String,
    #[schemars(description = "The JSON data to be inserted as the record content.")]
    pub data: serde_json::Map<String, serde_json::Value>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpsertParams {
    #[schemars(description = "Array of table names or record IDs to upsert.")]
    pub targets: Vec<String>,
    #[schemars(description = "The JSON patch operations to apply to the record or records.")]
    pub patch_data: Option<Vec<serde_json::Value>>,
    #[schemars(description = "The JSON data to combine with the existing record or records.")]
    pub merge_data: Option<serde_json::Map<String, serde_json::Value>>,
    #[schemars(description = "The JSON data to apply to the record or records.")]
    pub content_data: Option<serde_json::Map<String, serde_json::Value>>,
    #[schemars(description = "The JSON data to apply to the record or records.")]
    pub replace_data: Option<serde_json::Map<String, serde_json::Value>>,
    #[schemars(description = "Optional WHERE clause to filter records before upserting.")]
    pub where_clause: Option<String>,
    #[schemars(description = "Optional parameters to bind to the query.")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct UpdateParams {
    #[schemars(description = "Array of table names or record IDs to update.")]
    pub targets: Vec<String>,
    #[schemars(description = "The JSON patch operations to apply to the record or records.")]
    pub patch_data: Option<Vec<serde_json::Value>>,
    #[schemars(description = "The JSON data to combine with the existing record or records.")]
    pub merge_data: Option<serde_json::Map<String, serde_json::Value>>,
    #[schemars(description = "The JSON data to apply to the record or records.")]
    pub content_data: Option<serde_json::Map<String, serde_json::Value>>,
    #[schemars(description = "The JSON data to apply to the record or records.")]
    pub replace_data: Option<serde_json::Map<String, serde_json::Value>>,
    #[schemars(description = "Optional WHERE clause to filter records before upserting.")]
    pub where_clause: Option<String>,
    #[schemars(description = "Optional parameters to bind to the query.")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
}

#[derive(Deserialize, schemars::JsonSchema)]
pub struct DeleteParams {
    #[schemars(description = "Array of table names or record IDs to delete.")]
    pub targets: Vec<String>,
    #[schemars(description = "Optional WHERE clause to filter records before deletion.")]
    pub where_clause: Option<String>,
    #[schemars(description = "Optional parameters to bind to the query.")]
    pub parameters: Option<HashMap<String, serde_json::Value>>,
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
pub struct CloudParams {}

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
    /// Cloud client for SurrealDB Cloud operations
    pub cloud_client: Arc<Client>,
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
            cloud_client: Arc::new(Client::new()),
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
            has_bearer_token = false,
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
            cloud_client: Arc::new(Client::new()),
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

For security, you can use parameterized queries to prevent SQL injection by providing 
parameters that will be safely bound to the query. Use $param_name syntax in your query 
and provide the parameters in the parameters field.

The query results are returned as text, or an error occurs if the query execution fails.

Examples:
- SELECT * FROM person
- CREATE person:john CONTENT {name: "John", age: 30}
- UPDATE person SET age += 1 WHERE age < 30
- DELETE person WHERE age < 18
- RELATE person:john->knows->person:jane

Parameterized query examples:
- Query: "SELECT * FROM person WHERE age > $min_age AND name CONTAINS $name_filter"
  Parameters: {"min_age": 25, "name_filter": "John"}
- Query: "CREATE person:$id CONTENT {name: $name, age: $age}"
  Parameters: {"id": "john", "name": "John Doe", "age": 30}
"#)]
    pub async fn query(&self, params: Parameters<QueryParams>) -> Result<CallToolResult, McpError> {
        let QueryParams {
            query: query_string,
            parameters,
        } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.query").increment(1);
        // Output debugging information
        debug!(query_string = %query_string, "Executing SurrealQL query");
        // Convert tool parameters to SurrealQL parameters
        let parameters = if let Some(params) = parameters {
            let mut converted = HashMap::new();
            for (key, val) in params {
                let surreal_val = convert_json_to_surreal(val, &key)
                    .map_err(|e| McpError::internal_error(e, None))?;
                converted.insert(key, surreal_val);
            }
            Some(converted)
        } else {
            None
        };
        // Use the internal query function
        self.query_internal(query_string, parameters).await
    }

    /// Execute a SurrealDB SELECT statement to retrieve records from the database.
    ///
    /// This function executes a SurrealDB SELECT statement to query records from
    /// the specified tables or retrieve specific records by ID. Each item in the from
    /// parameter can be either a table name to select all records from that table, or a
    /// specific record ID in the format "table:id" to select a single record.
    /// The query results are returned as text, or an error occurs if the query
    /// execution fails.
    #[tool(description = r#"
Execute a SurrealDB SELECT statement to retrieve records from the database.

This function executes a SurrealDB SELECT statement to query records from the specified 
tables or record IDs. Each item in the what parameter is parsed to determine if it's a 
table name or a record ID. You can optionally add various clauses to filter, group, sort, 
and paginate the results.

Examples:
- select(["person"])  # All records from person table
- select(["person:john"])  # Specific record
- select(["person", "article"])  # All records from both tables
- select(["person:john", "article:123"])  # Specific records
- select(["person"], Some("age > 25"), None, None, Some("name ASC"), Some("10"), None)  # Filtered and sorted
- select(["person"], Some("age > $min_age"), None, Some("city"), Some("age DESC"), Some("10"), Some("20"), Some({"min_age": 25}))  # With parameters
- select(["article"], Some("published = true"), Some("author"), None, Some("created_at DESC"), Some("5"), None)  # With split and pagination
- select(["person"], Some("age > $min_age AND name CONTAINS $name_filter"), None, None, None, Some("10"), None, Some({ "min_age": 25, "name_filter": "John" }))  # Complex parameterized query
"#)]
    pub async fn select(
        &self,
        params: Parameters<SelectParams>,
    ) -> Result<CallToolResult, McpError> {
        let SelectParams {
            targets,
            where_clause,
            split_clause,
            group_clause,
            order_clause,
            limit_clause,
            start_clause,
            parameters,
        } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.select").increment(1);
        // Output debugging information
        debug!(targets = ?targets, "Selecting records");
        // Build the initial query string
        let mut query = "SELECT * FROM ".to_string();
        // Process the tables and Record IDs
        query.push_str(&parse_targets(targets).map_err(|e| McpError::internal_error(e, None))?);
        // Add the where clause if provided
        if let Some(v) = where_clause {
            query.push_str(&format!(" WHERE {v}"));
        }
        // Add the split on clause if provided
        if let Some(v) = split_clause {
            query.push_str(&format!(" SPLIT ON {v}"));
        }
        // Add the group by clause if provided
        if let Some(v) = group_clause {
            query.push_str(&format!(" GROUP BY {v}"));
        }
        // Add the order by clause if provided
        if let Some(v) = order_clause {
            query.push_str(&format!(" ORDER BY {v}"));
        }
        // Add the limit clause if provided
        if let Some(v) = limit_clause {
            query.push_str(&format!(" LIMIT BY {v}"));
        }
        // Add the start at clause if provided
        if let Some(v) = start_clause {
            query.push_str(&format!(" START AT {v}"));
        }
        // Create parameters with native SurrealDB types
        let mut params = HashMap::new();
        // Add user-provided parameters if any
        if let Some(variables) = parameters {
            for (key, val) in variables {
                let val = convert_json_to_surreal(val, &key)
                    .map_err(|e| McpError::internal_error(e, None))?;
                params.insert(key, val);
            }
        }
        // Output debugging information
        trace!("Selecting records with query: {query}");
        // Execute the final query
        self.query_internal(query, Some(params)).await
    }

    /// Insert new records into the specified tables or with specific record IDs.
    ///
    /// This function executes a SurrealDB INSERT statement to insert new records
    /// into the specified tables or with specific record IDs. The data is provided
    /// as a JSON value and will be used as the content for the new records.
    /// The INSERT statement is similar to CREATE but with different syntax.
    #[tool(description = r#"
Insert new records into the specified table or with specific record ID.

This function executes a SurrealDB INSERT statement to insert new records into the 
specified table or with specific record ID. The data is provided as an array of JSON objects 
and will be used as the content for the new records.

This is useful for batch inserting multiple records at once into a table.
The INSERT statement uses the syntax: INSERT [ IGNORE | RELATION ] INTO table [obj1, obj2, ...]

Examples:
- insert("person", [{"name": "Tobie", "age": 38}, {"name": "Jaime", "age": 40}])
- insert("article", [{"id": "article:123", "title": "New Article", "content": "Hello World"}])
- insert("person", [{"id": "jaime", "name": "Jaime"}], Some(true))  # With IGNORE
- insert("likes", [{"in": "person:1", "out": "person:2"}], None, Some(true))  # Relation table
"#)]
    pub async fn insert(
        &self,
        params: Parameters<InsertParams>,
    ) -> Result<CallToolResult, McpError> {
        let InsertParams {
            target,
            values,
            ignore,
            relation,
        } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.insert").increment(1);
        // Output debugging information
        debug!(target = %target, "Inserting records");
        // Build the initial query string
        let mut query = "INSERT ".to_string();
        // Add IGNORE keyword if specified
        if ignore.unwrap_or(false) {
            query.push_str("IGNORE ");
        }
        // Add RELATION keyword if specified
        if relation.unwrap_or(false) {
            query.push_str("RELATION ");
        }
        query.push_str("INTO ");
        // Process the table and Record ID
        query.push_str(&parse_target(target).map_err(|e| McpError::internal_error(e, None))?);
        // Add the data content clause
        query.push_str(" $data");
        // Create parameters with native SurrealDB types
        let mut params = HashMap::new();
        // Add the record data
        let values_array: Vec<serde_json::Value> =
            values.into_iter().map(serde_json::Value::Object).collect();
        let data = convert_json_to_surreal(serde_json::Value::Array(values_array), "data")
            .map_err(|e| McpError::internal_error(e, None))?;
        params.insert("data".to_string(), data);
        // Output debugging information
        trace!("Inserting records with query: {query}");
        // Execute the final query
        self.query_internal(query, Some(params)).await
    }

    /// Create a new record in the specified table with the provided data.
    ///
    /// This function executes a SurrealDB CREATE statement to insert a new record
    /// into the specified table. The data is provided as a JSON value and will be
    /// used as the content for the new record. The table parameter can be either
    /// a table name or a specific record ID in the format "table:id".
    #[tool(description = r#"
Create a new record in the specified tables or with specific record IDs.

This function executes a SurrealDB CREATE statement to insert a new record into the 
specified table or with specific record ID. The data is provided as a JSON value 
and will be used as the content for the new record.

This is useful for creating users, articles, products, or any other entity in your database.
"#)]
    pub async fn create(
        &self,
        params: Parameters<CreateParams>,
    ) -> Result<CallToolResult, McpError> {
        let CreateParams { target, data } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.create").increment(1);
        // Output debugging information
        debug!(target = ?target, "Creating record");
        // Build the initial query string
        let mut query = "CREATE ".to_string();
        // Process the tables and Record IDs
        query.push_str(&parse_target(target).map_err(|e| McpError::internal_error(e, None))?);
        // Add the data content clause
        query.push_str(" CONTENT $data");
        // Create parameters with native SurrealDB types
        let mut params = HashMap::new();
        // Add the record data
        let data =
            convert_json_to_surreal(data, "data").map_err(|e| McpError::internal_error(e, None))?;
        params.insert("data".to_string(), data);
        // Output debugging information
        trace!("Creating records with query: {query}");
        // Execute the final query
        self.query_internal(query, Some(params)).await
    }

    /// Execute a SurrealDB UPSERT statement to create or update records in the database.
    ///
    /// This function executes a SurrealDB UPSERT statement to create new records
    /// or update existing ones. The UPSERT statement combines the functionality
    /// of CREATE and UPDATE, inserting a new record if it doesn't exist, or
    /// updating an existing record if it does.
    #[tool(description = r#"
Execute a SurrealDB UPSERT statement to create or update records in the database.

This function executes a SurrealDB UPSERT statement to create new records or update 
existing ones. The UPSERT statement combines the functionality of CREATE and UPDATE, 
inserting a new record if it doesn't exist, or updating an existing record if it does.

Examples:
- upsert(["person:john"], {"name": "John", "age": 30})  # Creates or updates specific record
- upsert(["person"], {"name": "Jane", "age": 25}, Some("age > 18"))  # Upserts with condition
- upsert(["article:123"], {"title": "New Title"}, None, Some({"status": "published"}))  # Merge mode
"#)]
    pub async fn upsert(
        &self,
        params: Parameters<UpsertParams>,
    ) -> Result<CallToolResult, McpError> {
        let UpsertParams {
            targets,
            patch_data,
            merge_data,
            replace_data,
            content_data,
            where_clause,
            parameters,
        } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.upsert").increment(1);
        // Output debugging information
        debug!(targets = ?targets, "Upserting records");
        // Create parameters with native SurrealDB types
        let mut params = HashMap::new();
        // Build the initial query string
        let mut query = "UPSERT ".to_string();
        // Process the tables and Record IDs
        query.push_str(&parse_targets(targets).map_err(|e| McpError::internal_error(e, None))?);
        // Add the data content clause based on the mode
        match (replace_data, content_data, merge_data, patch_data) {
            (Some(v), None, None, None) => {
                query.push_str(" REPLACE $data");
                // Add the data input as a parameter
                params.insert(
                    "data".to_string(),
                    convert_json_to_surreal(v, "data")
                        .map_err(|e| McpError::internal_error(e, None))?,
                );
            }
            (None, Some(v), None, None) => {
                query.push_str(" CONTENT $data");
                // Add the data input as a parameter
                params.insert(
                    "data".to_string(),
                    convert_json_to_surreal(v, "data")
                        .map_err(|e| McpError::internal_error(e, None))?,
                );
            }
            (None, None, Some(v), None) => {
                query.push_str(" MERGE $data");
                // Add the data input as a parameter
                params.insert(
                    "data".to_string(),
                    convert_json_to_surreal(v, "data")
                        .map_err(|e| McpError::internal_error(e, None))?,
                );
            }
            (None, None, None, Some(v)) => {
                query.push_str(" PATCH $data");
                // Add the data input as a parameter
                params.insert(
                    "data".to_string(),
                    convert_json_to_surreal(v, "data")
                        .map_err(|e| McpError::internal_error(e, None))?,
                );
            }
            _ => {
                return Err(McpError::internal_error("Invalid upsert mode", None));
            }
        };
        // Add the where clause if provided
        if let Some(v) = where_clause {
            query.push_str(&format!(" WHERE {v}"));
        }
        // Add user-provided parameters if any
        if let Some(variables) = parameters {
            for (key, val) in variables {
                let val = convert_json_to_surreal(val, &key)
                    .map_err(|e| McpError::internal_error(e, None))?;
                params.insert(key, val);
            }
        }
        // Output debugging information
        trace!("Upserting records with query: {query}");
        // Execute the final query
        self.query_internal(query, Some(params)).await
    }

    /// Execute a SurrealDB UPDATE statement to modify records in the database.
    ///
    /// This function executes a SurrealDB UPDATE statement to modify the content
    /// of records in the database. The thing parameter can be either a table name
    /// to update all records in that table, or a specific record ID in the format
    /// "table:id" to update a single record. The update_mode parameter determines
    /// how the data is applied to the existing record.
    #[tool(description = r#"
Execute a SurrealDB UPDATE statement to modify records in the database.

This function executes a SurrealDB UPDATE statement to modify the content of records 
in the database. The what parameter accepts an array where each item can be either a 
table name or a specific record ID, similar to the select function.

Examples:
- update(["person"], {"age": 31})  # Updates all records in person table
- update(["person:john"], {"age": 31})  # Updates specific record
- update(["person", "article"], {"status": "active"})  # Updates records in multiple tables
- update(["person"], {"city": "NYC"}, Some("age > 25"), Some("merge"))  # Merges data for filtered records
- update(["article"], {"status": "published"}, Some("draft = true"))  # Updates draft articles
- update(["user"], {"last_login": "2024-01-15"}, Some("last_login < $cutoff_date"), Some("replace"), Some({ "cutoff_date": "2024-01-01" }))  # Parameterized query
"#)]
    pub async fn update(
        &self,
        params: Parameters<UpdateParams>,
    ) -> Result<CallToolResult, McpError> {
        let UpdateParams {
            targets,
            patch_data,
            merge_data,
            content_data,
            replace_data,
            where_clause,
            parameters,
        } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.update").increment(1);
        // Output debugging information
        debug!(targets = ?targets, "Updating records");
        // Create parameters with native SurrealDB types
        let mut params = HashMap::new();
        // Build the initial query string
        let mut query = "UPDATE ".to_string();
        // Process the tables and Record IDs
        query.push_str(&parse_targets(targets).map_err(|e| McpError::internal_error(e, None))?);
        // Add the data content clause
        match (replace_data, content_data, merge_data, patch_data) {
            (Some(v), None, None, None) => {
                query.push_str(" REPLACE $data");
                // Add the data input as a parameter
                params.insert(
                    "data".to_string(),
                    convert_json_to_surreal(v, "data")
                        .map_err(|e| McpError::internal_error(e, None))?,
                );
            }
            (None, Some(v), None, None) => {
                query.push_str(" CONTENT $data");
                // Add the data input as a parameter
                params.insert(
                    "data".to_string(),
                    convert_json_to_surreal(v, "data")
                        .map_err(|e| McpError::internal_error(e, None))?,
                );
            }
            (None, None, Some(v), None) => {
                query.push_str(" MERGE $data");
                // Add the data input as a parameter
                params.insert(
                    "data".to_string(),
                    convert_json_to_surreal(v, "data")
                        .map_err(|e| McpError::internal_error(e, None))?,
                );
            }
            (None, None, None, Some(v)) => {
                query.push_str(" PATCH $data");
                // Add the data input as a parameter
                params.insert(
                    "data".to_string(),
                    convert_json_to_surreal(v, "data")
                        .map_err(|e| McpError::internal_error(e, None))?,
                );
            }
            _ => {
                return Err(McpError::internal_error("Invalid update mode", None));
            }
        };
        // Add the where clause if provided
        if let Some(v) = where_clause {
            query.push_str(&format!(" WHERE {v}"));
        }
        // Add user-provided parameters if any
        if let Some(variables) = parameters {
            for (key, val) in variables {
                let val = convert_json_to_surreal(val, &key)
                    .map_err(|e| McpError::internal_error(e, None))?;
                params.insert(key, val);
            }
        }
        // Output debugging information
        trace!("Updating records with query: {query}");
        // Execute the final query
        self.query_internal(query, Some(params)).await
    }

    /// Execute a SurrealDB DELETE statement to remove records from the database.
    ///
    /// This function executes a SurrealDB DELETE statement to remove records from
    /// the specified table or delete a specific record by ID. The thing parameter
    /// can be either a table name to delete all records from that table, or a
    /// specific record ID in the format "table:id" to delete a single record.
    /// The query results are returned as text, or an error occurs if the query
    /// execution fails.
    #[tool(description = r#"
Execute a SurrealDB DELETE statement to remove records from the database.

This function executes a SurrealDB DELETE statement to remove records from the 
specified tables or specific record IDs. The what parameter accepts an array where 
each item can be either a table name or a specific record ID, similar to the select function.

Examples:
- delete(["person"])  # Deletes all records from person table
- delete(["person:john"])  # Deletes specific record
- delete(["person", "article"])  # Deletes all records from multiple tables
- delete(["person"], Some("age < 18"))  # Deletes records where age < 18
- delete(["article"], Some("published = false"))  # Deletes unpublished articles
- delete(["user"], Some("last_login < '2024-01-01'"))  # Deletes inactive users
- delete(["person"], Some("age > $min_age AND name CONTAINS $name_filter"), Some({ "min_age": 25, "name_filter": "John" }))  # Parameterized query
"#)]
    pub async fn delete(
        &self,
        params: Parameters<DeleteParams>,
    ) -> Result<CallToolResult, McpError> {
        let DeleteParams {
            targets,
            where_clause,
            parameters,
        } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.delete").increment(1);
        // Output debugging information
        debug!(targets = ?targets, "Deleting records");
        // Build the initial query string
        let mut query = "DELETE FROM ".to_string();
        // Process the tables and Record IDs
        query.push_str(&parse_targets(targets).map_err(|e| McpError::internal_error(e, None))?);
        // Add the where clause if provided
        if let Some(v) = where_clause {
            query.push_str(&format!(" WHERE {v}"));
        }
        // Create parameters with native SurrealDB types
        let mut params = HashMap::new();
        // Add user-provided parameters if any
        if let Some(variables) = parameters {
            for (key, val) in variables {
                let val = convert_json_to_surreal(val, &key)
                    .map_err(|e| McpError::internal_error(e, None))?;
                params.insert(key, val);
            }
        }
        // Output debugging information
        trace!("Deleting records with query: {query}");
        // Execute the final query
        self.query_internal(query, Some(params)).await
    }

    /// Create a relationship between two records in the database.
    ///
    /// This function executes a SurrealDB RELATE statement to create a relationship
    /// between two records. The relationship is defined by the from_id, relationship_type,
    /// and to_id parameters. Optionally, you can provide content data to store on the
    /// relationship edge itself.
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
        // Increment tool usage counter
        counter!("surrealmcp.tools.relate").increment(1);
        // Output debugging information
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

        self.query(Parameters(QueryParams {
            query,
            parameters: None,
        }))
        .await
    }

    #[tool(description = "List SurrealDB Cloud organizations")]
    pub async fn list_cloud_organizations(
        &self,
        _params: Parameters<CloudParams>,
    ) -> Result<CallToolResult, McpError> {
        // Increment tool usage counter
        counter!("surrealmcp.tools.list_cloud_organizations").increment(1);
        // Output debugging information
        debug!("Listing cloud organizations");
        // Fetch the cloud organisations
        let organisations = self
            .cloud_client
            .list_organizations()
            .await
            .or_else(|e| Err(McpError::internal_error(e.to_string(), None)))?;
        // Convert result to JSON
        let organisations: Vec<serde_json::Value> = organisations
            .into_iter()
            .map(|org| {
                serde_json::json!({
                    "id": org.id,
                    "name": org.name,
                    "slug": org.slug,
                    "created_at": org.created_at,
                    "updated_at": org.updated_at
                })
            })
            .collect();
        // Create the result JSON
        let result = serde_json::json!({
            "organizations": organisations,
            "count": organisations.len()
        });
        // Return the MCP result
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(description = "List SurrealDB Cloud instances for a given organization")]
    pub async fn list_cloud_instances(
        &self,
        params: Parameters<CloudOrganizationParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudOrganizationParams { organization_id } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.list_cloud_instances").increment(1);
        // Output debugging information
        debug!(
            organization_id = organization_id,
            "Listing cloud instances for organization"
        );
        // Fetch the cloud instances
        let instances = self
            .cloud_client
            .list_instances(&organization_id)
            .await
            .or_else(|e| Err(McpError::internal_error(e.to_string(), None)))?;
        // Convert result to JSON
        let instances: Vec<serde_json::Value> = instances
            .into_iter()
            .map(|instance| {
                serde_json::json!({
                    "id": instance.id,
                    "name": instance.name,
                    "status": instance.status,
                    "created_at": instance.created_at,
                    "updated_at": instance.updated_at
                })
            })
            .collect();
        // Create the result JSON
        let result = serde_json::json!({
            "instances": instances,
            "count": instances.len()
        });
        // Return the MCP result
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(description = "Pause SurrealDB Cloud instance")]
    pub async fn pause_cloud_instance(
        &self,
        params: Parameters<CloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudInstanceParams { instance_id } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.pause_cloud_instance").increment(1);
        // Output debugging information
        debug!(instance_id = instance_id, "Pausing cloud instance");
        // Pause the cloud instance
        let _ = self
            .cloud_client
            .pause_instance(&instance_id)
            .await
            .or_else(|e| Err(McpError::internal_error(e.to_string(), None)))?;
        // Create the result JSON
        let result = serde_json::json!({
            "message": "Successfully paused cloud instance",
            "instance_id": instance_id,
        });
        // Return the MCP result
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(description = "Resume SurrealDB Cloud instance")]
    pub async fn resume_cloud_instance(
        &self,
        params: Parameters<CloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudInstanceParams { instance_id } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.resume_cloud_instance").increment(1);
        // Output debugging information
        debug!(instance_id = instance_id, "Resuming cloud instance");
        // Pause the cloud instance
        let _ = self
            .cloud_client
            .resume_instance(&instance_id)
            .await
            .or_else(|e| Err(McpError::internal_error(e.to_string(), None)))?;
        // Create the result JSON
        let result = serde_json::json!({
            "message": "Successfully resumed cloud instance",
            "instance_id": instance_id,
        });
        // Return the MCP result
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(description = "Resume SurrealDB Cloud instance")]
    pub async fn get_cloud_instance_status(
        &self,
        params: Parameters<CloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudInstanceParams { instance_id } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.get_cloud_instance_status").increment(1);
        // Output debugging information
        debug!("Getting status for cloud instance: {instance_id}");
        // Fetch the cloud instance status
        let _ = self
            .cloud_client
            .get_instance_status(&instance_id)
            .await
            .or_else(|e| Err(McpError::internal_error(e.to_string(), None)))?;
        // Create the result JSON
        let result = serde_json::json!({
            "message": "Successfully fetched status for cloud instance",
            "instance_id": instance_id,
        });
        // Return the MCP result
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
    }

    #[tool(description = "Resume SurrealDB Cloud instance")]
    pub async fn get_cloud_instance_metrics(
        &self,
        params: Parameters<CloudInstanceParams>,
    ) -> Result<CallToolResult, McpError> {
        let CloudInstanceParams { instance_id } = params.0;
        // Increment tool usage counter
        counter!("surrealmcp.tools.get_cloud_instance_metrics").increment(1);
        // Output debugging information
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
        // Increment tool usage counter
        counter!("surrealmcp.tools.create_cloud_instance").increment(1);
        // Output debugging information
        debug!("Creating cloud instance: {name} in organization: {organization_id}");
        // Fetch the cloud instance status
        let instance = self
            .cloud_client
            .create_instance(&organization_id, &name)
            .await
            .or_else(|e| Err(McpError::internal_error(e.to_string(), None)))?;
        // Create the result JSON
        let result = serde_json::json!({
            "message": "Successfully created cloud instance",
            "instance": {
                "id": instance.id,
                "name": instance.name,
                "status": instance.status,
                "created_at": instance.created_at,
                "updated_at": instance.updated_at
            }
        });
        // Return the MCP result
        Ok(CallToolResult::success(vec![Content::text(
            result.to_string(),
        )]))
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
databases as needed. The connection is persistent until you disconnect or connect to 
a different endpoint. The username and password are optional.

Examples:
- connect_endpoint('memory')  # For testing
- connect_endpoint('file:/tmp/mydb', Some('myapp'), Some('production'))  # Local file storage
- connect_endpoint('ws://localhost:8000', Some('myapp'), Some('production'), Some('root'), Some('password'))  # Remote connection
- connect_endpoint('rocksdb:/data/mydb', Some('analytics'), Some('events'))  # High-performance local storage
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
        // Start the measurement timer
        let start_time = Instant::now();
        // Increment tool usage counter
        counter!("surrealmcp.tools.connect_endpoint").increment(1);
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
                // Increment error metrics
                counter!("surrealmcp.total_errors").increment(1);
                counter!("surrealmcp.total_configuration_errors").increment(1);
                counter!("surrealmcp.errors.connect_endpoint").increment(1);
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
                // Calculate the elapsed time
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
                // Calculate the elapsed time
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
                counter!("surrealmcp.total_errors").increment(1);
                counter!("surrealmcp.total_connection_errors").increment(1);
                counter!("surrealmcp.errors.connect_endpoint").increment(1);
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
        // Start the measurement timer
        let start_time = Instant::now();
        // Increment tool usage counter
        counter!("surrealmcp.tools.use_namespace").increment(1);
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
                // Increment error metrics
                counter!("surrealmcp.total_errors").increment(1);
                counter!("surrealmcp.total_configuration_errors").increment(1);
                counter!("surrealmcp.errors.use_namespace").increment(1);
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
                        counter!("surrealmcp.total_errors").increment(1);
                        counter!("surrealmcp.total_connection_errors").increment(1);
                        counter!("surrealmcp.errors.use_namespace").increment(1);
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
                // Increment error metrics
                counter!("surrealmcp.total_errors").increment(1);
                counter!("surrealmcp.total_configuration_errors").increment(1);
                counter!("surrealmcp.errors.no_connection").increment(1);
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
        // Start the measurement timer
        let start_time = Instant::now();
        // Increment tool usage counter
        counter!("surrealmcp.tools.use_database").increment(1);
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
                // Increment error metrics
                counter!("surrealmcp.total_errors").increment(1);
                counter!("surrealmcp.total_configuration_errors").increment(1);
                counter!("surrealmcp.errors.use_database").increment(1);
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
                        counter!("surrealmcp.total_errors").increment(1);
                        counter!("surrealmcp.total_connection_errors").increment(1);
                        counter!("surrealmcp.errors.use_database").increment(1);
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
                // Increment error metrics
                counter!("surrealmcp.total_errors").increment(1);
                counter!("surrealmcp.total_configuration_errors").increment(1);
                counter!("surrealmcp.errors.no_connection").increment(1);
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
        // Increment tool usage metrics
        counter!("surrealmcp.tools.disconnect_endpoint").increment(1);
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

    /// Internal query function that executes a SurrealQL query.
    ///
    /// This function accepts SurrealDB native Value types, allowing for direct use of
    /// SurrealQL parameters without JSON conversion. This is used by other tools to
    /// avoid JSON conversion overhead.
    async fn query_internal(
        &self,
        query_string: String,
        parameters: Option<HashMap<String, Value>>,
    ) -> Result<CallToolResult, McpError> {
        // Increment the query counter
        let query_id = QUERY_COUNTER.fetch_add(1, Ordering::SeqCst);
        // Lock the database connection
        let db_guard = self.db.lock().await;
        // Match the database connection
        match &*db_guard {
            Some(db) => {
                // Execute the query on the engine
                let res = engine::execute_query(
                    db,
                    query_id,
                    query_string,
                    parameters,
                    &self.connection_id,
                )
                .await;
                // Check the result of the query
                match res {
                    Ok(response) => {
                        // Convert response to MCP result
                        response.to_mcp_result()
                    }
                    Err(e) => {
                        // Return the received error message
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
                counter!("surrealmcp.total_errors").increment(1);
                counter!("surrealmcp.total_configuration_errors").increment(1);
                counter!("surrealmcp.errors.no_connection").increment(1);
                // Return error message
                Err(McpError::internal_error(
                    "Not connected to any SurrealDB endpoint. Use connect_endpoint first."
                        .to_string(),
                    None,
                ))
            }
        }
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
                    // Increment error metrics
                    counter!("surrealmcp.total_errors").increment(1);
                    counter!("surrealmcp.total_connection_errors").increment(1);
                    counter!("surrealmcp.errors.connect_endpoint").increment(1);
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
    /// Get the MCP server info
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

    /// Initialize the MCP server
    async fn initialize(
        &self,
        _req: rmcp::model::InitializeRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::InitializeResult, McpError> {
        debug!("Initializing MCP server");
        // Get the bearer token from the extensions
        if let Some(parts) = _ctx.extensions.get::<Parts>() {
            if let Some(token) = parts.extensions.get::<String>() {
                self.cloud_client
                    .client_token
                    .write()
                    .await
                    .replace(token.clone());
            }
        }
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

    /// List the MCP server prompts
    async fn list_prompts(
        &self,
        _req: Option<rmcp::model::PaginatedRequestParam>,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::ListPromptsResult, McpError> {
        // Output debugging information
        debug!("Listing available prompts");
        // Get prompts from the prompts module
        let prompts = prompts::get_available_prompts();
        // Return the prompts
        Ok(rmcp::model::ListPromptsResult {
            prompts,
            next_cursor: None,
        })
    }

    /// Get an MCP server prompt
    async fn get_prompt(
        &self,
        req: rmcp::model::GetPromptRequestParam,
        _ctx: RequestContext<RoleServer>,
    ) -> Result<rmcp::model::GetPromptResult, McpError> {
        // Output debugging information
        debug!(prompt_name = %req.name, "Getting prompt");
        // Get prompt from the prompts module
        match prompts::get_prompt_with_arguments(&req.name, req.arguments) {
            Some((description, messages)) => Ok(rmcp::model::GetPromptResult {
                description: Some(description),
                messages,
            }),
            None => Err(McpError::internal_error(
                format!("Unknown prompt: {}", req.name),
                None,
            )),
        }
    }
}
