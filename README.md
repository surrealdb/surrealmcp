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

```bash
cargo install --path .
```

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
export SURREAL_MCP_JWE_DECRYPTION_KEY="base64-encoded-32-byte-key"
export SURREAL_MCP_CLOUD_ACCESS_TOKEN="your_access_token_here"
export SURREAL_MCP_CLOUD_REFRESH_TOKEN="your_refresh_token_here"

surrealmcp start
```

## Authentication

The server supports Bearer token authentication with SurrealDB Cloud. When authentication is enabled:

1. **JWT Tokens**: Validates JWT tokens using JWKS (JSON Web Key Set) from the auth server
2. **JWE Tokens**: Validates JWE headers and tokens (when a decryption key is specified)
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

### JWE Decryption Key Configuration

For JWE (JSON Web Encryption) tokens, you can specify a base64-encoded decryption key:

```bash
# Set a JWE decryption key
surrealmcp start --jwe-decryption-key "base64-encoded-32-byte-key"

# Or via environment variable
export SURREAL_MCP_JWE_DECRYPTION_KEY="base64-encoded-32-byte-key"
surrealmcp start
```

This is useful when:
- Your application uses JWE tokens for enhanced security
- You need to decrypt and validate the full token contents
- You're working with encrypted tokens that require a specific decryption key

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

This project is licensed under the Apache 2.0 License.