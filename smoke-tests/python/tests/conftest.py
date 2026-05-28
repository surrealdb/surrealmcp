from __future__ import annotations

import sys
from pathlib import Path

import pytest

WORKSPACE_ROOT = Path(__file__).resolve().parents[1]
if str(WORKSPACE_ROOT) not in sys.path:
    sys.path.insert(0, str(WORKSPACE_ROOT))

from smoke.config import SmokeSettings, load_settings
from smoke.mcp_client import SurrealMcpStdioClient


@pytest.fixture(scope="session")
def smoke_settings() -> SmokeSettings:
    return load_settings()


@pytest.fixture
def mcp_client(smoke_settings: SmokeSettings) -> SurrealMcpStdioClient:
    return SurrealMcpStdioClient(smoke_settings)
