# SurrealDB MCP Server

You are connected to a SurrealDB MCP server that provides tools for interacting with a SurrealDB database. SurrealDB is a multi-model database that supports document, graph, and relational data models.

## Database Connection

This MCP server uses SurrealDB's `Any` engine, which dynamically selects the storage backend based on the `SURREALDB_URL` environment variable. **Each MCP client connection gets its own isolated SurrealDB connection**, ensuring complete separation between different LLM sessions.

Supported backends include:

- **Memory**: `memory` - In-memory database (no persistence)
- **File**: `file:/path/to/db` - Local file storage
- **RocksDB**: `rocksdb:/path/to/db` - High-performance local storage
- **TiKV**: `tikv://localhost:2379` - Distributed storage
- **Remote WebSocket**: `ws://localhost:8000` - WebSocket connection
- **Remote HTTP**: `http://localhost:8000` - HTTP connection

The server automatically detects the engine type and handles authentication appropriately. Each client connection is completely isolated from others.

## Connection Workflow

1. **Initial state**: When you first connect to the MCP server, no database connection is established
2. **Connect**: Use `connect_endpoint` to establish a connection to your desired SurrealDB endpoint
3. **Query**: Execute SurrealQL queries using any of the available tools
4. **Switch**: Use `connect_endpoint` again to switch to a different endpoint
5. **Disconnect**: Use `disconnect_endpoint` to close the current connection

### Example workflow

```rust
// 1. Connect to an in-memory database
connect_endpoint("memory", None, None, None, None)

// 2. Create some data
create("person", {"name": "John", "age": 30})

// 3. Switch to a file-based database
connect_endpoint("file:/tmp/mydb", Some("myapp"), Some("production"), None, None)

// 4. Query the new database
select("person")

// 5. Disconnect when done
disconnect_endpoint()
```

## Available tools

### Basic operations
- **query**: Execute raw SurrealQL queries for maximum flexibility
- **create**: Insert new records into tables
- **select**: Retrieve records from tables or specific record IDs
- **update**: Modify records with support for replace, merge, and patch modes
- **relate**: Add relationships between records (graph relationships)
- **delete**: Remove records from tables

### Connection operations
- **connect_endpoint**: Connect to a different SurrealDB endpoint
- **disconnect_endpoint**: Disconnect from the current SurrealDB endpoint

### Cloud management
- **list_cloud_instances**: List Surreal Cloud instances
- **pause_cloud_instance**: Pause a cloud instance
- **resume_cloud_instance**: Resume a cloud instance
- **create_cloud_instance**: Create a new cloud instance

## Key concepts

### Record IDs
SurrealDB uses the format `table:id` for record identifiers:
- `person:john` - Record with ID "john" in "person" table
- `article:surreal_intro` - Record with ID "surreal_intro" in "article" table

### Update modes
The `update` tool supports three modes:
- **replace** (default): Replaces entire record content
- **merge**: Merges new data with existing data
- **patch**: Applies JSON patch operations

### Relationships
Use `relate` to create graph relationships:
- `person:john -> wrote -> article:surreal_intro`
- `person:john -> knows -> person:jane`

## Best practices

1. **Use specific record IDs** when you know them for better performance
2. **Use the raw query tool** for complex operations not covered by convenience functions
3. **Use merge/patch modes** when updating records to preserve existing data
4. **Create relationships** to model graph data and enable complex queries
5. **Use table names** in select/delete when you want to operate on all records

## Example workflows

### Creating a blog system
1. Create authors: `create("person", {"name": "John", "email": "john@example.com"})`
2. Create articles: `create("article", {"title": "SurrealDB Guide", "content": "..."})`
3. Link them: `relate("person:john", "wrote", "article:surreal_guide", None)`

### Updating user profiles
1. Replace entire profile: `update("person:john", {"name": "John", "age": 30}, None)`
2. Add new fields: `update("person:john", {"city": "NYC"}, Some("merge"))`
3. Update specific field: `update("person:john", [{"op": "replace", "path": "/age", "value": 31}], Some("patch"))`

## SurrealQL reference

**IMPORTANT**: SurrealQL is NOT similar to ANSI-SQL. **Never** assume ANSI-SQL or SQL knowledge from other databases applies to SurrealQL. Always refer to the official SurrealDB website at https://surrealdb.com or the documentation at https://surrealdb.com/docs for accurate syntax and behavior.

### Key differences from SQL

- SurrealQL uses different syntax for many operations
- Every row in a table is called a Record
- Each Record in a table has a unique `id` field which can not be changed once specified
- A Record is similar to a MongoDB document, with the ability to store nested objects and arrays
- Record IDs use `table:id` format (e.g., `person:john`)
- Relationships are expressed with the following syntax:
    - `->` arrows for traversing outbound graph connections
    - `<-` arrows for traversing inbound graph connections
    - `<->` arros for traversing a graph connection in any direction
- Many operations work differently than in traditional SQL

### Best practices for SurrealQL

1. **Always use specific Record IDs** when you know them for better performance
2. **Use relationships** to model graph data instead of foreign keys
3. **Use MERGE and PATCH** for updates to preserve existing data
4. **Define schemas** for data validation and consistency
5. **Use indexes** on frequently queried fields
6. **Use LIVE queries** for real-time updates when needed

### Basic SurrealQL statements

#### SELECT statement

```surql
-- Select all records from a table
SELECT * FROM person;

-- Select specific fields
SELECT name, age FROM person;

-- Select by record ID
SELECT * FROM person:john;

-- Select with WHERE conditions
SELECT * FROM person WHERE age > 25;

-- Select with ORDER BY and LIMIT
SELECT * FROM person WHERE age > 25 ORDER BY name LIMIT 10;

-- Select with relationships (graph queries)
SELECT * FROM person WHERE ->knows->person.age > 30;

-- Select with nested relationships
SELECT * FROM person WHERE ->wrote->article->has->category.name = 'Technology';

-- Select with aggregation
SELECT count() FROM person;
SELECT count() FROM article GROUP BY author;

-- Select with subqueries
SELECT * FROM person WHERE id IN (SELECT author FROM article);
```

#### CREATE statement

```surql
-- Create a single record
CREATE person:john CONTENT {
    name: 'John Doe',
    age: 30,
    email: 'john@example.com'
};

-- Create multiple records
CREATE person CONTENT [
    { name: 'Alice', age: 25 },
    { name: 'Bob', age: 35 }
];

-- Create with specific ID
CREATE person:alice CONTENT { name: 'Alice', age: 25 };
```

#### UPDATE statement

```surql
-- Replace entire record content
UPDATE person:john CONTENT {
    name: 'John Doe',
    age: 31,
    city: 'New York'
};

-- Merge new data with existing data
UPDATE person:john MERGE {
    age: 31,
    city: 'New York'
};

-- Apply JSON patch operations
UPDATE person:john PATCH [
    { op: 'replace', path: '/age', value: 31 },
    { op: 'add', path: '/city', value: 'New York' }
];

-- Update multiple records
UPDATE person SET age += 1 WHERE age < 30;
```

#### DELETE statement

```surql
-- Delete a specific record
DELETE person:john;

-- Delete all records from a table
DELETE person;

-- Delete with conditions
DELETE person WHERE age < 18;
```

#### RELATE statement

```surql
-- Create a simple relationship
RELATE person:john->knows->person:jane;

-- Create relationship with content on the edge
RELATE person:john->wrote->article:surreal_guide CONTENT {
    date: '2024-01-15',
    word_count: 1500
};

-- Create multiple relationships
RELATE person:john->knows->person:jane;
RELATE person:john->knows->person:bob;
```

#### Configure schema

```surql
-- Define a table
DEFINE TABLE person SCHEMAFULL;

-- Define fields on a table
DEFINE FIELD name ON TABLE person TYPE string;
DEFINE FIELD age ON TABLE person TYPE int;
DEFINE FIELD email ON TABLE person TYPE string;

-- Define indexes
DEFINE INDEX idx_name ON TABLE person COLUMNS name;
DEFINE INDEX idx_age ON TABLE person COLUMNS age;

-- Define roles and permissions
DEFINE ROLE admin ON TABLE person PERMISSIONS ALL;
DEFINE ROLE user ON TABLE person PERMISSIONS SELECT, UPDATE WHERE id = $auth.id;
```

#### Real-time subscriptions

```surql
-- Set up live query for all person records
LIVE SELECT * FROM person;

-- Live query with conditions
LIVE SELECT * FROM person WHERE age > 25;

-- Live query for relationships
LIVE SELECT * FROM person WHERE ->knows->person.age > 30;
```

### Advanced SurrealQL features

#### Array and Object Operations

```surql
-- Access nested object properties
SELECT name, profile.city FROM person;

-- Array operations
SELECT * FROM person WHERE tags CONTAINS 'developer';

-- Object operations
SELECT * FROM person WHERE profile.age > 25;
```

#### Time and date operations

```surql
-- Time-based queries
SELECT * FROM article WHERE created_at > '2024-01-01';

-- Date functions
SELECT * FROM event WHERE time::day(created_at) = time::day(now());
```

#### String operations

```surql
-- String matching
SELECT * FROM person WHERE name CONTAINS 'John';

-- String functions
SELECT * FROM person WHERE string::uppercase(name) = 'JOHN';
```

### Common Patterns

#### Blog System Example

```sql
-- Create authors and articles
CREATE person:john CONTENT { name: 'John Doe', email: 'john@example.com' };
CREATE article:surreal_guide CONTENT { title: 'SurrealDB Guide', content: '...' };

-- Create relationships
RELATE person:john->wrote->article:surreal_guide;
RELATE article:surreal_guide->has->category:technology;

-- Query articles by author
SELECT * FROM article WHERE ->wrote->person.name = 'John Doe';

-- Query authors by article category
SELECT * FROM person WHERE ->wrote->article->has->category.name = 'Technology';
```

The server is ready to help you work with your SurrealDB database! 