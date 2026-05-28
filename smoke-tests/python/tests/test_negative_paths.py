"""Negative-path smoke tests for SurrealMCP stdio interactions.

These tests validate error handling for malformed queries, invalid tool calls,
missing required parameters, unreachable endpoints, and authentication failures.
"""

from __future__ import annotations

from dataclasses import replace

import pytest
from mcp.shared.exceptions import McpError
from smoke.config import SmokeSettings
from smoke.mcp_client import SurrealMcpStdioClient


@pytest.mark.anyio
async def test_query_tool_returns_parse_error_for_invalid_surrealql(
    mcp_client: SurrealMcpStdioClient,
) -> None:
    """Ensure malformed SurrealQL propagates an MCP parse error."""
    async with mcp_client.connect():
        await mcp_client.initialize()

        with pytest.raises(McpError, match=r"(?i)parse error"):
            await mcp_client.call_tool(
                "query",
                {"query": "CREATE type::thing('bad', 'query') CONTENT { a: 1 };"},
            )


@pytest.mark.anyio
async def test_use_namespace_requires_namespace_parameter(
    mcp_client: SurrealMcpStdioClient,
) -> None:
    """Ensure use_namespace rejects calls that omit the namespace parameter."""
    async with mcp_client.connect():
        await mcp_client.initialize()

        with pytest.raises(McpError, match=r"(?i)missing field `namespace`"):
            await mcp_client.call_tool("use_namespace", {})


@pytest.mark.anyio
async def test_unknown_tool_name_returns_tool_not_found(
    mcp_client: SurrealMcpStdioClient,
) -> None:
    """Ensure unknown tool names return a tool-not-found MCP error."""
    async with mcp_client.connect():
        await mcp_client.initialize()

        with pytest.raises(McpError, match=r"(?i)tool not found"):
            await mcp_client.call_tool("tool_that_does_not_exist", {})


@pytest.mark.anyio
async def test_unreachable_startup_endpoint_reports_no_connection_and_connect_failure(
    smoke_settings: SmokeSettings,
) -> None:
    """Ensure unreachable endpoints fail both startup query and explicit connect flows."""
    unreachable_settings = replace(smoke_settings, surrealdb_url="ws://127.0.0.1:65535/rpc")
    mcp_client = SurrealMcpStdioClient(unreachable_settings)

    async with mcp_client.connect():
        await mcp_client.initialize()

        with pytest.raises(McpError, match=r"(?i)not connected to any surrealdb endpoint"):
            await mcp_client.call_tool("query", {"query": "SELECT * FROM smoke_e2e LIMIT 1;"})

        with pytest.raises(McpError, match=r"(?i)failed to connect to endpoint"):
            await mcp_client.call_tool(
                "connect_endpoint",
                {
                    "endpoint": unreachable_settings.surrealdb_url,
                    "namespace": unreachable_settings.surrealdb_ns,
                    "database": unreachable_settings.surrealdb_db,
                    "username": unreachable_settings.surrealdb_user,
                    "password": unreachable_settings.surrealdb_pass,
                },
            )


@pytest.mark.anyio
async def test_connect_endpoint_with_wrong_credentials_fails_authentication(
    mcp_client: SurrealMcpStdioClient,
    smoke_settings: SmokeSettings,
) -> None:
    """Ensure connect_endpoint surfaces authentication failures for bad credentials."""
    async with mcp_client.connect():
        await mcp_client.initialize()

        with pytest.raises(McpError, match=r"(?i)failed to connect to endpoint.*authentication"):
            await mcp_client.call_tool(
                "connect_endpoint",
                {
                    "endpoint": smoke_settings.surrealdb_url,
                    "namespace": smoke_settings.surrealdb_ns,
                    "database": smoke_settings.surrealdb_db,
                    "username": smoke_settings.surrealdb_user,
                    "password": f"{smoke_settings.surrealdb_pass}_invalid",
                },
            )
