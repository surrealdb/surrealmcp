<br>

<p align="center">
    <img width=120 src="https://raw.githubusercontent.com/surrealdb/icons/main/surreal.svg" />
    &nbsp;
    <img width=120 src="https://raw.githubusercontent.com/surrealdb/icons/main/mcp.svg" />
</p>

<h3 align="center">The official MCP server for SurrealDB.</h3>

<br>

<p align="center">
    <a href="https://github.com/surrealdb/surrealmcp"><img src="https://img.shields.io/badge/status-preview-ff00bb.svg?style=flat-square"></a>
    &nbsp;
    <a href="https://github.com/surrealdb/surrealmcp"><img src="https://img.shields.io/github/v/release/surrealdb/surrealmcp?color=9600FF&include_prereleases&label=version&sort=semver&style=flat-square"></a>
    &nbsp;
    <a href="https://surrealdb.com/docs/integrations/data-management/n8n"><img src="https://img.shields.io/badge/docs-view-44cc11.svg?style=flat-square"></a>
    &nbsp;
    <a href="https://github.com/surrealdb/license"><img src="https://img.shields.io/badge/license-BSL_1.1-00bfff.svg?style=flat-square"></a>
</p>

<p align="center">
    <a href="https://surrealdb.com/discord"><img src="https://img.shields.io/discord/902568124350599239?label=discord&style=flat-square&color=5a66f6"></a>
    &nbsp;
    <a href="https://twitter.com/surrealdb"><img src="https://img.shields.io/badge/twitter-follow_us-1d9bf0.svg?style=flat-square"></a>
    &nbsp;
    <a href="https://www.linkedin.com/company/surrealdb/"><img src="https://img.shields.io/badge/linkedin-connect_with_us-0a66c2.svg?style=flat-square"></a>
    &nbsp;
    <a href="https://www.youtube.com/@SurrealDB"><img src="https://img.shields.io/badge/youtube-subscribe-fc1c1c.svg?style=flat-square"></a>
</p>

# SurrealMCP

SurrealMCP is the official Model Context Protocol ([MCP](https://modelcontextprotocol.io)) server for SurrealDB and SurrealDB Cloud that enables AI assistants, AI agents, Developer IDEs, AI chatbots, and data platforms to interact with SurrealDB databases and SurrealDB Cloud.

## Features

- **Multiple transport modes**: Support for `stdio`, HTTP, and Unix socket connections
- **Authentication**: Bearer token authentication with SurrealDB Cloud
- **Rate limiting**: Configurable request rate limiting
- **Health checks**: Built-in health checking
- **Structured logging**: Comprehensive logging and metrics
- **OpenTelemetry support**: Support for `stdio` and OpenTelemetry tracing
- **SurrealDB endpoint lockdown**: Enable connecting to a specific SurrealDB endpoint only

## Installation

#### Building from source

```bash
cargo install --path .
```

#### Deploying with Docker

```bash
docker run --rm -i --pull always surrealdb/surrealmcp:latest start
```

<!-- -------------------------------------------------- -->
<!-- AI tools -->
<!-- -------------------------------------------------- -->

## AI coding tools integration

SurrealMCP can be integrated with various AI coding tools and assistants to enable AI-powered database operations. Below are installation and configuration instructions for popular AI coding platforms.

### Which AI assistant are you using?

- **Using [Cursor](https://www.cursor.com)?** → [View the Cursor installation instructions](#cursor-installation)
- **Using [Claude Desktop](https://claude.ai)?** → [View the Claude installation instructions](#claude-installation)
- **Using [GitHub Copilot](https://github.com/copilot) in VS Code?** → [View the Copilot installation instructions](#copilot-installation)
- **Using [Zed](https://zed.dev)?** → [View the Zed installation instructions](#zed-installation)
- **Using [n8n](https://n8n.io)?** → [View the n8n integration instructions](#integration-with-n8n)

### Key Terms

- **MCP Server**: A server that implements the Model Context Protocol, allowing AI assistants to access external tools and resources.
- **MCP Client**: The IDE, application (like Cursor, Zed, or Claude Desktop) that connects to MCP servers.
- **[SurrealDB](https://surrealdb.com)**: A scalable, distributed, document-graph database with real-time capabilities.

<!-- -------------------------------------------------- -->
<!-- Cursor -->
<!-- -------------------------------------------------- -->

### Cursor installation

#### Installation for Cursor

1. **Install SurrealMCP:**
  - [Build and install from source](#installation)
  - [Configure with Docker](#deployment-with-docker)

2. **Configure Cursor:**
   - Open Cursor
   - Go to Settings > Cursor Settings
   - Find the MCP Servers option and enable it
   - Click on "New MCP Server"

3. **Add the SurrealMCP configuration:**
   ```json
   {
     "name": "SurrealDB",
     "command": "docker",
     "args": [
       "run",
       "--rm",
       "-i",
       "--pull", "always",
       "surrealdb/surrealmcp:latest",
       "start"
     ]
   }
   ```

   <details>
   <summary>Configuration with environment variables</summary>

   ```json
   {
     "name": "SurrealDB",
     "command": "surrealmcp",
     "args": ["start"],
     "env": {
       "SURREALDB_URL": "ws://localhost:8000/rpc",
       "SURREALDB_NS": "myapp",
       "SURREALDB_DB": "production",
       "SURREALDB_USER": "admin",
       "SURREALDB_PASS": "password123"
     }
   }
   ```
   </details>

4. **Verify installation:**
   - Open Cursor Chat
   - You should see SurrealDB tools available in the tools list

<!-- -------------------------------------------------- -->
<!-- Claude -->
<!-- -------------------------------------------------- -->

### Claude installation

#### Installation for Claude Desktop App

1. **Install SurrealMCP:**
  - [Build and install from source](#installation)
  - [Configure with Docker](#deployment-with-docker)

2. **Configure Claude Desktop:**

   Edit the Claude Desktop App's MCP settings file:
   - Windows: `%APPDATA%\Claude\claude_desktop_config.json`
   - macOS: `~/Library/Application Support/Claude/claude_desktop_config.json`
   - Linux: `~/.config/Claude/claude_desktop_config.json`

   Add the following configuration:
   ```json
   {
     "mcpServers": {
       "SurrealDB": {
         "command": "docker",
         "args": [
           "run",
           "--rm",
           "-i",
           "--pull", "always",
           "surrealdb/surrealmcp:latest",
           "start"
         ],
         "disabled": false,
         "autoApprove": []
       }
     }
   }
   ```

   <details>
   <summary>Configuration with environment variables</summary>

   ```json
   {
     "mcpServers": {
       "SurrealDB": {
         "command": "surrealmcp",
         "args": ["start"],
         "env": {
           "SURREALDB_URL": "ws://localhost:8000/rpc",
           "SURREALDB_NS": "myapp",
           "SURREALDB_DB": "production",
           "SURREALDB_USER": "admin",
           "SURREALDB_PASS": "password123"
         },
         "disabled": false,
         "autoApprove": []
       }
     }
   }
   ```
   </details>

3. **Verify installation:**
   - Ask Claude to "list available MCP servers"
   - You should see "SurrealDB" in the list

<!-- -------------------------------------------------- -->
<!-- Copilot -->
<!-- -------------------------------------------------- -->

### Copilot installation

#### Installation for GitHub Copilot in VS Code

1. **Install SurrealMCP:**
  - [Build and install from source](#installation)
  - [Configure with Docker](#deployment-with-docker)

2. **Configure VSCode:**
   
   Create a file at: `.vscode/mcp.json` in your workspace

   Add the following configuration:
   ```json
   {
     "servers": {
       "SurrealDB": {
         "type": "stdio",
         "command": "docker",
         "args": [
           "run",
           "--rm",
           "-i",
           "--pull", "always",
           "surrealdb/surrealmcp:latest",
           "start"
         ]
       }
     }
   }
   ```

   <details>
   <summary>Configuration with environment variables</summary>

   ```json
   {
     "inputs": [
       {
         "type": "promptString",
         "id": "surrealdb-url",
         "description": "SurrealDB URL",
         "default": "ws://localhost:8000/rpc"
       },
       {
         "type": "promptString",
         "id": "surrealdb-ns",
         "description": "SurrealDB Namespace"
       },
       {
         "type": "promptString",
         "id": "surrealdb-db",
         "description": "SurrealDB Database"
       },
       {
         "type": "promptString",
         "id": "surrealdb-user",
         "description": "SurrealDB Username"
       },
       {
         "type": "promptString",
         "id": "surrealdb-pass",
         "description": "SurrealDB Password",
         "password": true
       }
     ],
     "servers": {
       "SurrealDB": {
         "type": "stdio",
         "command": "surrealmcp",
         "args": ["start"],
         "env": {
           "SURREALDB_URL": "${input:surrealdb-url}",
           "SURREALDB_NS": "${input:surrealdb-ns}",
           "SURREALDB_DB": "${input:surrealdb-db}",
           "SURREALDB_USER": "${input:surrealdb-user}",
           "SURREALDB_PASS": "${input:surrealdb-pass}"
         }
       }
     }
   }
   ```
   </details>

3. **Verify installation:**
   - Open GitHub Copilot Chat in VS Code
   - Select "Agent" mode from the dropdown
   - Click the "Tools" button to see available tools
   - You should see "SurrealDB" tools in the list

<!-- -------------------------------------------------- -->
<!-- Zed -->
<!-- -------------------------------------------------- -->

### Zed installation

#### Installation for Zed

1. **Install SurrealMCP:**
  - [Build and install from source](#installation)
  - [Configure with Docker](#deployment-with-docker)

2. **Add the SurrealMCP configuration:**

   ```json
   "surreal": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-i",
        "--pull", "always",
        "surrealdb/surrealmcp:latest",
        "start"
      ],
      "enabled": true,
    }
   ```

   <details>
   <summary>Configuration with environment variables</summary>

   ```json
   "surreal": {
      "command": "surrealmcp",
      "args": ["start"],
      "enabled": true,
      "env": {
        "SURREALDB_URL": "ws://localhost:8000/rpc",
        "SURREALDB_NS": "myapp",
        "SURREALDB_DB": "production",
        "SURREALDB_USER": "admin",
        "SURREALDB_PASS": "password123"
      }
    }
   ```
   </details>

<!-- -------------------------------------------------- -->
<!-- n8n -->
<!-- -------------------------------------------------- -->

### Integration with n8n

You can integrate SurrealMCP with [n8n](https://n8n.io/) using the [MCP Client Tool](https://docs.n8n.io/integrations/builtin/cluster-nodes/sub-nodes/n8n-nodes-langchain.toolmcp/) node.

<!-- -------------------------------------------------- -->
<!-- Docker -->
<!-- -------------------------------------------------- -->

### Deployment with Docker

#### Using Docker with STDIO

To use SurrealMCP without installing `cargo` you can use Docker. SurrealMCP runs locally via stdio with a single Docker command. Instantly start an in-memory or local database at the edge directly from your AI tool, with ephemeral or persisted data. 

```json
{
  "mcpServers": {
    "SurrealDB": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-i",
        "--pull", "always",
        "surrealdb/surrealmcp:latest",
        "start"
      ]
    }
  }
}
```

#### Using Docker with HTTP

To use SurrealMCP without installing `cargo` you can use Docker. SurrealMCP can be run as an HTTP server with a single Docker command. Instantly start an in-memory or local database at the edge directly from your AI tool, with ephemeral or persisted data. 

```json
{
  "mcpServers": {
    "SurrealDB": {
      "command": "docker",
      "args": [
        "run",
        "--rm",
        "-p", "8080:8080",
        "--pull", "always",
        "surrealdb/surrealmcp:latest",
        "start",
        "--bind-address", "127.0.0.1:8080",
        "--server-url", "http://localhost:8080"
      ]
    }
  }
}
```
[!IMPORTANT]
If you are using Docker Desktop, you may need to use `host.docker.internal` instead of `localhost` when specifying the SurrealDB instance URL for the MCP server to connect to.

## Usage

### Basic Usage

```bash
# Start as a STDIO server (default)
surrealmcp start

# Start as a HTTP server
surrealmcp start --bind-address 127.0.0.1:8000

# Start as a Unix socket
surrealmcp start --socket-path /tmp/surrealmcp.sock
```

### Configuration Options

```bash
# Database connection
surrealmcp start \
  --endpoint ws://localhost:8000/rpc \
  --ns mynamespace \
  --db mydatabase \
  --user root \
  --pass root

# Server configuration
surrealmcp start \
  --bind-address 127.0.0.1:8000 \
  --server-url https://mcp.surrealdb.com \
  --cloud-auth-server https://auth.surrealdb.com \
  --expected-audience https://custom.audience.com/ \
  --rate-limit-rps 100 \
  --rate-limit-burst 200

# Disable authentication (for development)
surrealmcp start --bind-address 127.0.0.1:8000 --auth-disabled
```

### Environment Variables

All configuration options can be set via environment variables:

```bash
export SURREALDB_URL="ws://localhost:8000/rpc"
export SURREALDB_NS="mynamespace"
export SURREALDB_DB="mydatabase"
export SURREALDB_USER="root"
export SURREALDB_PASS="root"
export SURREAL_MCP_BIND_ADDRESS="127.0.0.1:8000"
export SURREAL_MCP_SERVER_URL="https://mcp.surrealdb.com"
export SURREAL_CLOUD_AUTH_SERVER="https://auth.surrealdb.com"
export SURREAL_MCP_EXPECTED_AUDIENCE="https://custom.audience.com/"
export SURREAL_MCP_RATE_LIMIT_RPS="100"
export SURREAL_MCP_RATE_LIMIT_BURST="200"
export SURREAL_MCP_AUTH_REQUIRED="false"
export SURREAL_MCP_CLOUD_ACCESS_TOKEN="your_access_token_here"
export SURREAL_MCP_CLOUD_REFRESH_TOKEN="your_refresh_token_here"

surrealmcp start
```

## Authentication

The server supports Bearer token authentication with SurrealDB Cloud. When authentication is enabled:

1. **JWT Tokens**: Validates JWT tokens using JWKS (JSON Web Key Set) from the auth server
2. **JWE Tokens**: Validates JWE header structure and issuer
3. **Audience validation**: Validates the `aud` claim against the expected audience
4. **Issuer validation**: Validates the `iss` claim against the expected issuer

### Custom Audience Configuration

You can specify a custom expected audience for JWT token validation:

```bash
# Set a custom audience
surrealmcp start --expected-audience "https://myapp.com/api"

# Or via environment variable
export SURREAL_MCP_EXPECTED_AUDIENCE="https://myapp.com/api"
surrealmcp start
```

This is useful when:
- Your application uses a custom audience in JWT tokens
- You want to restrict tokens to specific applications
- You're integrating with custom authentication systems



### Pre-configured Cloud Authentication Tokens

For SurrealDB Cloud operations, you can provide pre-configured access and refresh tokens instead of fetching them dynamically:

```bash
# Set pre-configured tokens
surrealmcp start \
  --access-token "your_access_token_here" \
  --refresh-token "your_refresh_token_here"

# Or via environment variables
export SURREAL_MCP_CLOUD_ACCESS_TOKEN="your_access_token_here"
export SURREAL_MCP_CLOUD_REFRESH_TOKEN="your_refresh_token_here"
surrealmcp start
```

This is useful when:
- You have existing tokens from a previous authentication flow
- You want to avoid the token fetching process
- You're running in environments where token fetching is not possible
- You want to use long-lived tokens for automated operations

**Note**: When both access and refresh tokens are provided, the server will use these tokens for all SurrealDB Cloud API operations instead of attempting to fetch new tokens.

### Client Integration

When integrating with the MCP server, clients should:

1. **Discover the authorization configuration**:
   ```bash
   curl http://localhost:8000/.well-known/oauth-protected-resource
   ```

2. **Request a token from the authorization server** using the returned audience:
   ```bash
   # Example Auth0 token request
   curl -X POST https://auth.surrealdb.com/oauth/token \
     -H "Content-Type: application/json" \
     -d '{
       "client_id": "your-client-id",
       "client_secret": "your-client-secret",
       "audience": "https://custom.audience.com/",
       "grant_type": "client_credentials"
     }'
   ```

3. **Use the token** for authenticated requests:
   ```bash
   curl -H "Authorization: Bearer YOUR_TOKEN" \
     http://localhost:8000/mcp
   ```

The audience value ensures that tokens are issued specifically for the MCP server instance.

## Available Tools

SurrealMCP provides a comprehensive set of tools for interacting with SurrealDB databases and SurrealDB Cloud:

### Database Operations

- **Query**: Execute raw SurrealQL queries with parameterized inputs
- **Select**: Query records with filtering, sorting, and pagination
- **Insert**: Insert new records into tables
- **Create**: Create single records with specific IDs
- **Upsert**: Create or update records based on conditions
- **Update**: Modify existing records with patch operations
- **Delete**: Remove records from the database
- **Relate**: Create relationships between records

### Connection Management

- **Connect Endpoint**: Connect to different SurrealDB endpoints including:
  - Local instances: `memory`, `file:/path`, `rocksdb:/path`
  - Remote instances: `ws://host:port`, `http://host:port`
  - SurrealDB Cloud instances: `cloud:instance_id`
- **Use Namespace**: Switch between namespaces
- **Use Database**: Switch between databases
- **List Namespaces**: List the defined namespaces
- **List Databases**: List the defined databases
- **Disconnect Endpoint**: Close the current connection

### SurrealDB Cloud Operations

- **List Cloud Organizations**: Get available organizations
- **List Cloud Instances**: Get instances for an organization
- **Create Cloud Instance**: Create new cloud instances
- **Pause/Resume Cloud Instance**: Manage instance lifecycle
- **Get Cloud Instance Status**: Check instance health and backups

### Cloud Connection Feature

The new cloud connection feature allows you to connect directly to SurrealDB Cloud instances using the `connect_endpoint` tool with the `cloud:instance_id` format:

```bash
# Connect to a SurrealDB Cloud instance
connect_endpoint('cloud:abc123def456', 'myapp', 'production')
```

This feature:
- Automatically fetches authentication tokens from the SurrealDB Cloud API
- Validates instance readiness before connecting
- Establishes secure connections using the temporary auth token
- Supports namespace and database specification
- Handles connection errors gracefully with detailed logging

## API Endpoints

### Health Check

```bash
curl http://localhost:8000/health
```

### Authentication Discovery

```bash
curl http://localhost:8000/.well-known/oauth-protected-resource
```

This endpoint returns the authorization server configuration including the expected audience:

```json
{
  "resource": "https://mcp.surrealdb.com",
  "authorization_servers": ["https://auth.surrealdb.com"],
  "bearer_methods_supported": ["header"],
  "audience": "https://mcp.surrealdb.com/"
}
```

Clients should use the `audience` value when requesting tokens from the authorization server.

### MCP Protocol

The server implements the Model Context Protocol and can be used with MCP-compatible clients.

## Development

### Building

```bash
cargo build
```

### Testing

```bash
cargo test
```

### Running with Docker

```bash
docker build -t surrealmcp .
docker run -p 8000:8000 surrealmcp start --bind-address 0.0.0.0:8000
```

## License

This project is licensed under the [Business Source License](LICENSE).
