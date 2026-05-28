from __future__ import annotations

import json

import pytest

from smoke.config import SmokeSettings
from smoke.mcp_client import SurrealMcpStdioClient, extract_text_content


def _assert_named_items(payload: dict[str, object], key: str) -> None:
    values = payload.get(key)
    assert isinstance(values, list)
    for value in values:
        assert isinstance(value, dict)
        assert isinstance(value.get("name"), str)

    count = payload.get("count")
    assert isinstance(count, int)
    assert count == len(values)


def _parse_tool_json(result_text: str) -> dict[str, object]:
    payload = json.loads(result_text)
    assert isinstance(payload, dict)
    return payload


@pytest.mark.anyio
async def test_tools_list_contains_metadata_tools(
    mcp_client: SurrealMcpStdioClient,
) -> None:
    async with mcp_client.connect():
        await mcp_client.initialize()
        result = await mcp_client.list_tools()

        tool_names = {tool.name for tool in result.tools}
        assert "list_namespaces" in tool_names
        assert "list_databases" in tool_names


@pytest.mark.anyio
async def test_list_namespaces_payload_shape(
    mcp_client: SurrealMcpStdioClient,
) -> None:
    async with mcp_client.connect():
        await mcp_client.initialize()
        result = await mcp_client.call_tool("list_namespaces")

        payload = _parse_tool_json(extract_text_content(result))
        _assert_named_items(payload, "namespaces")


@pytest.mark.anyio
async def test_list_databases_payload_shape(
    mcp_client: SurrealMcpStdioClient,
) -> None:
    async with mcp_client.connect():
        await mcp_client.initialize()
        result = await mcp_client.call_tool("list_databases")

        payload = _parse_tool_json(extract_text_content(result))
        _assert_named_items(payload, "databases")


@pytest.mark.anyio
async def test_metadata_tools_include_configured_namespace_and_database(
    mcp_client: SurrealMcpStdioClient,
    smoke_settings: SmokeSettings,
) -> None:
    async with mcp_client.connect():
        await mcp_client.initialize()

        namespace_result = await mcp_client.call_tool("list_namespaces")
        namespace_payload = _parse_tool_json(extract_text_content(namespace_result))
        _assert_named_items(namespace_payload, "namespaces")

        namespaces = namespace_payload["namespaces"]
        assert isinstance(namespaces, list)
        namespace_names = {
            item["name"] for item in namespaces if isinstance(item, dict) and isinstance(item.get("name"), str)
        }
        assert smoke_settings.surrealdb_ns in namespace_names

        database_result = await mcp_client.call_tool("list_databases")
        database_payload = _parse_tool_json(extract_text_content(database_result))
        _assert_named_items(database_payload, "databases")

        databases = database_payload["databases"]
        assert isinstance(databases, list)
        database_names = {
            item["name"] for item in databases if isinstance(item, dict) and isinstance(item.get("name"), str)
        }
        assert smoke_settings.surrealdb_db in database_names


@pytest.mark.anyio
async def test_switch_namespace_and_database_tools(
    mcp_client: SurrealMcpStdioClient,
    smoke_settings: SmokeSettings,
) -> None:
    async with mcp_client.connect():
        await mcp_client.initialize()

        namespace_result = await mcp_client.call_tool("use_namespace", {"namespace": smoke_settings.surrealdb_ns})
        namespace_text = extract_text_content(namespace_result)
        assert smoke_settings.surrealdb_ns in namespace_text
        assert "Successfully switched" in namespace_text

        database_result = await mcp_client.call_tool("use_database", {"database": smoke_settings.surrealdb_db})
        database_text = extract_text_content(database_result)
        assert smoke_settings.surrealdb_db in database_text
        assert "Successfully switched" in database_text
