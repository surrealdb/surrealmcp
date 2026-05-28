from __future__ import annotations

from contextlib import asynccontextmanager
from typing import AsyncIterator

import mcp.types as types
from mcp import ClientSession
from mcp.client.stdio import StdioServerParameters, stdio_client

from smoke.config import SmokeSettings


class SurrealMcpStdioClient:
    def __init__(self, settings: SmokeSettings):
        self._settings = settings
        self._session: ClientSession | None = None

    @asynccontextmanager
    async def connect(self) -> AsyncIterator["SurrealMcpStdioClient"]:
        server = StdioServerParameters(
            command=self._settings.server.command,
            args=self._settings.server.args,
            env=self._settings.server_env(),
            cwd=self._settings.server.cwd,
        )
        async with stdio_client(server) as (read_stream, write_stream):
            async with ClientSession(read_stream, write_stream) as session:
                self._session = session
                try:
                    yield self
                finally:
                    self._session = None

    async def initialize(self) -> types.InitializeResult:
        return await self._require_session().initialize()

    async def send_initialized_notification(self) -> None:
        notification = types.ClientNotification(types.InitializedNotification())
        await self._require_session().send_notification(notification)

    async def list_tools(self) -> types.ListToolsResult:
        return await self._require_session().list_tools()

    async def call_tool(
        self,
        name: str,
        arguments: dict[str, object] | None = None,
    ) -> types.CallToolResult:
        return await self._require_session().call_tool(name, arguments)

    def _require_session(self) -> ClientSession:
        if self._session is None:
            raise RuntimeError("MCP client session has not been opened.")
        return self._session


def extract_text_content(result: types.CallToolResult) -> str:
    for item in result.content:
        text = getattr(item, "text", None)
        if isinstance(text, str):
            return text
    raise RuntimeError("CallToolResult did not contain text content.")
