from __future__ import annotations

import pytest

from smoke.mcp_client import SurrealMcpStdioClient


@pytest.mark.anyio
async def test_initialize_and_initialized_notification(
    mcp_client: SurrealMcpStdioClient,
) -> None:
    async with mcp_client.connect():
        initialize_result = await mcp_client.initialize()

        assert initialize_result.protocolVersion
        assert initialize_result.serverInfo.name
        assert initialize_result.serverInfo.version
        assert initialize_result.capabilities.tools is not None

        # initialize() already sends notifications/initialized; this call
        # verifies explicit notification sending through the SDK as well.
        await mcp_client.send_initialized_notification()
