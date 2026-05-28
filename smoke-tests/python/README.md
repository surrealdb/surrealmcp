# SurrealMCP Python Smoke Tests

This workspace contains uv-managed smoke tests for SurrealMCP using the official
MCP Python SDK package `mcp` over stdio transport.

## Prerequisites

- `uv` installed locally
- A running SurrealDB instance (local default from `.env` is `ws://127.0.0.1:8000/rpc`)
- Access to this repository so the smoke tests can launch `surrealmcp start`

## Environment Variables

The smoke tests load `.env` automatically using `python-dotenv`.

Current local defaults:

```dotenv
SURREALDB_URL=ws://127.0.0.1:8000/rpc
SURREALDB_NS=test
SURREALDB_DB=test
SURREALDB_USER=root
SURREALDB_PASS=root
```

Optional override for server launch command:

- `SURREALMCP_SERVER_COMMAND`: full command used to start the MCP server
  (example: `cargo run --manifest-path ../../Cargo.toml -- start`)

When `SURREALMCP_SERVER_COMMAND` is not set, the smoke client uses:

1. `target/debug/surrealmcp start` when available
2. otherwise `cargo run --quiet --manifest-path <repo>/Cargo.toml -- start`

## Setup

```bash
uv sync
```

## Run Smoke Tests

Run full suite:

```bash
uv run pytest -q
```

Run through the helper script:

```bash
uv run python run_smoke.py
```

Run a single test module:

```bash
uv run pytest -q tests/test_metadata_tools.py
```

Pass custom pytest args through helper script:

```bash
uv run python run_smoke.py tests/test_stdio_handshake.py -vv
```

## Current Test Inventory

The harness currently runs 12 tests across 4 test modules.

### tests/test_stdio_handshake.py

- test_initialize_and_initialized_notification
  - Verifies MCP initialize succeeds and returns protocol version, server name, server version, and tool capabilities.
  - Sends an explicit initialized notification after initialize to validate notification flow.

### tests/test_metadata_tools.py

- test_tools_list_contains_metadata_tools
  - Verifies tools/list includes both list_namespaces and list_databases.

- test_list_namespaces_payload_shape
  - Verifies list_namespaces returns JSON with namespaces as a list of objects containing name and count matching list length.

- test_list_databases_payload_shape
  - Verifies list_databases returns JSON with databases as a list of objects containing name and count matching list length.

- test_metadata_tools_include_configured_namespace_and_database
  - Verifies configured SURREALDB_NS exists in list_namespaces output.
  - Verifies configured SURREALDB_DB exists in list_databases output.

- test_switch_namespace_and_database_tools
  - Verifies use_namespace succeeds for configured namespace and returns a success message.
  - Verifies use_database succeeds for configured database and returns a success message.

### tests/test_query_roundtrip.py

- test_query_tool_create_select_delete_roundtrip
  - Creates a unique record via query tool.
  - Selects the created record and verifies unique key and marker are present.
  - Executes a parameterized query and verifies parameter binding works.
  - Deletes the created record in cleanup.

### tests/test_negative_paths.py

- test_query_tool_returns_parse_error_for_invalid_surrealql
  - Verifies invalid SurrealQL raises MCP parse error.

- test_use_namespace_requires_namespace_parameter
  - Verifies missing required namespace argument raises parameter deserialization error.

- test_unknown_tool_name_returns_tool_not_found
  - Verifies unknown tool invocation raises tool not found error.

- test_unreachable_startup_endpoint_reports_no_connection_and_connect_failure
  - Verifies unreachable startup endpoint leaves session unconnected and query calls fail with not connected error.
  - Verifies connect_endpoint to unreachable endpoint returns failed to connect error.

- test_connect_endpoint_with_wrong_credentials_fails_authentication
  - Verifies connect_endpoint with incorrect credentials returns authentication failure.

## Verify Collected Tests

To inspect exactly what pytest will collect:

```bash
uv run pytest --collect-only -q
```
