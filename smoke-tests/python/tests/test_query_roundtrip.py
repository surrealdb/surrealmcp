from __future__ import annotations

from uuid import uuid4

import pytest

from smoke.mcp_client import SurrealMcpStdioClient, extract_text_content


@pytest.mark.anyio
async def test_query_tool_create_select_delete_roundtrip(
    mcp_client: SurrealMcpStdioClient,
) -> None:
    record_key = f"smoke_{uuid4().hex[:10]}"
    marker = f"marker_{uuid4().hex[:8]}"

    create_query = f"CREATE smoke_e2e:{record_key} CONTENT {{ marker: '{marker}', n: 42, source: 'smoke' }};"
    select_query = f"SELECT * FROM smoke_e2e:{record_key};"
    parameterized_query = "SELECT * FROM smoke_e2e WHERE marker = $marker LIMIT 1;"
    delete_query = f"DELETE smoke_e2e:{record_key};"

    async with mcp_client.connect():
        await mcp_client.initialize()

        try:
            create_result = await mcp_client.call_tool("query", {"query": create_query})
            create_text = extract_text_content(create_result)
            assert "smoke_e2e" in create_text
            assert record_key in create_text
            assert marker in create_text

            select_result = await mcp_client.call_tool("query", {"query": select_query})
            select_text = extract_text_content(select_result)
            assert "smoke_e2e" in select_text
            assert record_key in select_text
            assert marker in select_text

            parameterized_result = await mcp_client.call_tool(
                "query",
                {"query": parameterized_query, "parameters": {"marker": marker}},
            )
            parameterized_text = extract_text_content(parameterized_result)
            assert record_key in parameterized_text
            assert marker in parameterized_text
        finally:
            await mcp_client.call_tool("query", {"query": delete_query})
