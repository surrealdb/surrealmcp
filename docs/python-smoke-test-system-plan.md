# Python Smoke Test System Plan (UV)

## Purpose

Create a UV-managed Python smoke-test system to validate SurrealMCP end-to-end behavior against a running SurrealDB instance, with deterministic MCP stdio checks implemented through the official MCP Python SDK (`mcp`) and clear local execution workflows.

## Current Status (As Implemented)

This plan has been implemented for the stdio smoke-test scope.

Completed:

1. UV-managed smoke-test workspace created at `/home/sasan/projects/mcp-proxy/surrealmcp/smoke-tests/python`.
2. Dependencies added: `pytest`, `mcp`, `python-dotenv`.
3. Local `.env` created with `SURREALDB_NS=test` and local SurrealDB defaults.
4. MCP stdio helper client implemented using official MCP Python SDK transport/session APIs.
5. Test suite expanded beyond initial handshake and metadata scope to include:
   - query roundtrip assertions
   - negative-path/error assertions
6. Local runner script implemented (`run_smoke.py`).
7. Workspace-local `.gitignore` implemented in smoke-test folder only.
8. Smoke-test README updated with setup, execution, and full test inventory.
9. Root `.gitignore` left unchanged.

Current suite status:

1. Total tests: 12
2. Modules:
   - `tests/test_stdio_handshake.py`
   - `tests/test_metadata_tools.py`
   - `tests/test_query_roundtrip.py`
   - `tests/test_negative_paths.py`

## Decisions

1. Smoke-test workspace location: `/home/sasan/projects/mcp-proxy/surrealmcp/smoke-tests/python`.
2. Package and environment management: `uv` only.
3. Test framework: `pytest`.
4. MCP client library for all smoke tests: official MCP Python SDK package `mcp`.
5. Include a local CLI runner script for one-command execution.
6. Do not modify repository root `.gitignore`.
7. Add ignore rules only in `/home/sasan/projects/mcp-proxy/surrealmcp/smoke-tests/python/.gitignore`.
8. CI workflow edits are out of scope for this batch.
9. Include a workspace-local `.env` file for smoke tests using the current local SurrealDB connection values and `SURREALDB_NS=test`.
10. Use `python-dotenv` to load `.env` in smoke-test Python code (no shell-based env sourcing required).

## Verified MCP SDK Transport Findings

1. Official package: `mcp` (PyPI package for the official Python MCP SDK).
2. HTTP support: yes. Streamable HTTP transport is supported and documented as the recommended production transport.
3. SSE support: yes, and currently documented as being superseded by Streamable HTTP.
4. WebSocket support: yes, transport modules exist for client/server in the SDK.
5. WebSocket dependency model: install WebSocket extra when needed (`mcp[ws]`), since WebSocket support is optional.
6. Planning implication for this workspace: keep stdio-first tests in this batch, prioritize Streamable HTTP for next transport expansion, and treat WebSocket tests as optional follow-up coverage.

## Scope

In scope:

1. UV project bootstrap for smoke tests.
2. Pytest-based MCP smoke tests (stdio transport first).
3. MCP helper code built on MCP SDK transport/session utilities (no custom JSON-RPC wire parser in tests).
4. Local runner script.
5. Workspace-local `.env` file for test execution.
6. Dotenv-based configuration loading in smoke-test Python code.
7. Folder-local `.gitignore`.
8. Usage documentation inside the smoke-test workspace.

Out of scope:

1. Changes to root `.gitignore`.
2. CI workflow changes.
3. Release pipeline changes.
4. New production server functionality.

## Proposed Folder Layout

```text
smoke-tests/python/
  .env
  .gitignore
  pyproject.toml
  uv.lock
  README.md
  run_smoke.py
   smoke/__init__.py
   smoke/mcp_client.py
   smoke/config.py
   tests/conftest.py
   tests/test_stdio_handshake.py
   tests/test_metadata_tools.py
   tests/test_query_roundtrip.py
   tests/test_negative_paths.py
```

## Implementation Status

1. [x] Create the folder structure under `smoke-tests/python`.
2. [x] Initialize the UV project with Python 3.11.
3. [x] Add `smoke-tests/python/.env` with current local SurrealDB values and test namespace:
   ```dotenv
   SURREALDB_URL=ws://127.0.0.1:8000/rpc
   SURREALDB_NS=test
   SURREALDB_DB=test
   SURREALDB_USER=root
   SURREALDB_PASS=root
   ```
   - Source for these values: running local SurrealDB process (`surreal start ... --user root --pass root`) listening on `127.0.0.1:8000`.
4. [x] Add test dependencies:
   - Required: `pytest`, `mcp`, `python-dotenv`
   - Optional (follow-up): `requests` if HTTP assertions are added later
   - Optional (follow-up): `mcp[ws]` if WebSocket smoke tests are added
5. [x] Generate `uv.lock`.
6. [x] Add `smoke/mcp_client.py`:
   - Start `surrealmcp` stdio process using MCP SDK client transport helpers
   - Create and manage an MCP SDK client session
   - Provide helper methods for `initialize`, `tools/list`, and `tools/call` using SDK APIs
7. [x] Add `smoke/config.py` for environment-driven test settings:
   - Load `.env` using `python-dotenv` (`load_dotenv`) at startup
   - Endpoint, namespace, database, username, password
   - Binary/command path if needed
8. [x] Add pytest tests:
   - `tests/conftest.py`
     - Shared fixture to create/teardown an MCP SDK client session
   - `test_stdio_handshake.py`
     - initialize request via SDK session
     - `notifications/initialized` via SDK session
     - successful MCP server response validation
   - `test_metadata_tools.py`
     - `tools/list` includes `list_namespaces` and `list_databases`
     - `tools/call` for `list_namespaces` returns JSON with:
       - `namespaces` (array of objects with `name`)
       - `count` (integer)
     - `tools/call` for `list_databases` returns JSON with:
       - `databases` (array of objects with `name`)
       - `count` (integer)
   - Additional implemented coverage:
     - `test_query_roundtrip.py`
       - create/select/delete roundtrip with parameterized query assertions
     - `test_negative_paths.py`
       - invalid SurrealQL parse error
       - missing required tool parameter error
       - unknown tool error
       - unreachable endpoint behavior
       - wrong-credentials authentication behavior
9. [x] Add `run_smoke.py`:
   - Load `.env` with `python-dotenv` before invoking pytest
   - Execute pytest programmatically or via subprocess
   - Pass through optional pytest args
   - Return proper non-zero exit code on failure
10. [x] Add folder-local `.gitignore` at `smoke-tests/python/.gitignore` with entries such as:

- `.venv/`
- `.pytest_cache/`
- `.ruff_cache/`
- `.env`
- `__pycache__/`
- `*.pyc`

11. [x] Add `smoke-tests/python/README.md` with:
    - prerequisites
    - environment variables
    - setup commands
    - local run commands

- current test inventory

## Local Run Workflow

1. Create/sync environment:
   - `uv sync`
2. Run full suite:
   - `uv run pytest -q`
3. Run via helper script:
   - `uv run python run_smoke.py`
4. Run targeted tests:
   - `uv run pytest -q tests/test_metadata_tools.py`
5. Inspect collected tests:
   - `uv run pytest --collect-only -q`
6. Environment loading behavior:
   - `.env` is loaded by smoke-test Python code via `python-dotenv`.

## Validation Results (Executed)

1. UV bootstrap:
   - `uv sync` succeeded in `smoke-tests/python`.
2. `.env` namespace:
   - `SURREALDB_NS=test` present in `smoke-tests/python/.env`.
3. MCP SDK import:
   - `uv run python -c "import mcp"` succeeded.
4. Dotenv import:
   - `uv run python -c "from dotenv import load_dotenv"` succeeded.
5. Test collection:
   - `uv run pytest --collect-only -q` reports 12 tests.
6. Test execution:
   - `uv run pytest -q` passes.
   - `uv run python run_smoke.py` passes.
7. Negative-path validation:
   - explicit failures are asserted for parse errors, missing parameters, unknown tools, unreachable endpoint, and bad credentials.
8. Ignore-file scope:
   - no changes to repository root `.gitignore`.
   - ignore entries are contained in `smoke-tests/python/.gitignore`.

## Acceptance Criteria Status

1. [x] A complete UV-based Python smoke-test workspace exists in `smoke-tests/python`.
2. [x] Tests cover MCP initialize flow and metadata tools.
3. [x] All smoke tests use official MCP Python SDK package `mcp` (no custom wire-level JSON-RPC client in tests).
4. [x] Workspace contains a `.env` file with local SurrealDB values and `SURREALDB_NS=test`.
5. [x] Smoke-test Python code loads `.env` via `python-dotenv`.
6. [x] Runner script provides one-command execution.
7. [x] Root `.gitignore` remains unchanged.
8. [x] Folder-local `.gitignore` exists and handles local artifacts.
9. [x] Smoke tests are runnable locally with documented commands.

## Follow-Up Options

1. Add Streamable HTTP transport smoke tests using session-id flow.
2. Add optional WebSocket transport smoke tests using `mcp[ws]`.
3. Add CI job once local stability is proven.
4. Add disconnect-endpoint coverage and multi-step reconnect assertions.
