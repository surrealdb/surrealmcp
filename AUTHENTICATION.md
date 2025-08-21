# Authentication

This document explains the current token validation implementation and what you need to do to achieve full validation including all claims (audience, expiration, issued at, subject).

## How authentication works

1. **Middleware integration**
   - Axum middleware for HTTP endpoints
   - Proper 401 responses with WWW-Authenticate headers
   - Health and metadata endpoints bypass authentication

2. **JWE Token header validation**
   - Validates the 5-part structure (header.encrypted_key.iv.ciphertext.tag)
   - Checks algorithm (`dir`), encryption (`A256GCM`), and issuer (`https://auth.surrealdb.com/`)
   - Validates the header structure and issuer

3. **JWT Token full validation**
   - Validates 3-part structure (header.payload.signature)
   - Validates issuer, audience, expiration, and issued at claims
   - Supports RSA and EC algorithms

4. **JWE Token full validation**
   - We can only validate the header structure without the decryption key
   - The actual claims (audience, expiration, issued at, subject) are encrypted
   - When a decryption key is provided we decrypt the token and validate the claims
   - Full validation requires the decryption key from SurrealDB
   
## How to perform full token validation

1. **JWE Token Validation**

JWE tokens are validated by checking the header structure and issuer. The server validates that the token uses the expected algorithm ("dir") and encryption method ("A256GCM") and that the issuer matches the expected value.

## Configuration pptions

The SurrealMCP server supports various authentication configuration options that can be specified via command-line arguments or environment variables:

### Server URL

Specify the local server URL for authentication callback:

- **Command-line argument**: `--server_url`
- **Environment Variable**: `SURREAL_MCP_SERVER_URL`
- **Default**: `https://mcp.surrealdb.com`

**Example:**
```bash
# Command-line
surrealmcp --server_url "http://localhost:8000"

# Environment variable
export SURREAL_MCP_SERVER_URL="http://localhost:8000"
surrealmcp
```

### Authentication server

Specify the authentication server URL:

- **Command-line argument**: `--auth_server`
- **Environment Variable**: `SURREAL_MCP_AUTH_SERVER`
- **Default**: `https://auth.surrealdb.com`

**Example:**
```bash
# Command-line
surrealmcp --auth_server "https://auth.surrealdb.com"

# Environment variable
export SURREAL_MCP_AUTH_SERVER="https://auth.surrealdb.com"
surrealmcp
```

### Authentication audience

Specify the audience for authentication tokens:

- **Command-line argument**: `--auth_audience`
- **Environment Variable**: `SURREAL_MCP_AUTH_AUDIENCE`
- **Default**: `https://mcp.surrealdb.com`

**Example:**
```bash
# Command-line
surrealmcp --auth_audience "https://your-mcp-server.com"

# Environment variable
export SURREAL_MCP_AUTH_AUDIENCE="https://your-mcp-server.com"
surrealmcp
```

### Disabling authentication

To disable authentication completely (this will also disable calling SurrealDB Cloud methods):

- **Command-line argument**: `--auth_disabled`
- **Environment Variable**: `SURREAL_MCP_AUTH_DISABLED`
- **Default**: `false`

**Example:**
```bash
# Command-line
surrealmcp --auth_disabled

# Environment variable
export SURREAL_MCP_AUTH_DISABLED=true
surrealmcp
```

### Complete configuration example

Here's an example of using multiple authentication options together:

```bash
# Using command-line arguments
surrealmcp \
  --server_url "http://localhost:8000" \
  --auth_server "https://auth.surrealdb.com" \
  --auth_audience "https://your-mcp-server.com"

# Using environment variables
export SURREAL_MCP_SERVER_URL="http://localhost:8000"
export SURREAL_MCP_AUTH_SERVER="https://auth.surrealdb.com"
export SURREAL_MCP_AUTH_AUDIENCE="https://your-mcp-server.com"
surrealmcp
```

**Note:** When authentication is disabled (`--auth_disabled` or `SURREAL_MCP_AUTH_DISABLED=true`), the server will not validate any tokens and will not be able to call SurrealDB Cloud methods. This is useful for local development or when using a local SurrealDB instance.

## Security considerations

1. **Decryption key security**
   - Store the decryption key securely (environment variables, secure key management)
   - Never commit the key to version control
   - Rotate keys regularly

2. **Clock Skew**
   - The current implementation includes a 5-minute clock skew for token validation
   - Adjust based on your deployment environment

3. **Audience Validation**
   - Ensure the audience matches your MCP server's URL
   - Prevents token reuse across different services

4. **Expiration Validation**
   - Always validate token expiration
   - Consider implementing token refresh mechanisms