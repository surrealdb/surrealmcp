from __future__ import annotations

import os
import shlex
from dataclasses import dataclass
from pathlib import Path

from dotenv import load_dotenv

SMOKE_WORKSPACE = Path(__file__).resolve().parents[1]
REPO_ROOT = Path(__file__).resolve().parents[3]
DOTENV_PATH = SMOKE_WORKSPACE / ".env"

load_dotenv(DOTENV_PATH, override=False)


@dataclass(frozen=True)
class ServerProcessConfig:
    command: str
    args: list[str]
    cwd: Path


@dataclass(frozen=True)
class SmokeSettings:
    surrealdb_url: str
    surrealdb_ns: str
    surrealdb_db: str
    surrealdb_user: str
    surrealdb_pass: str
    server: ServerProcessConfig

    def server_env(self) -> dict[str, str]:
        env = os.environ.copy()
        env.update(
            {
                "SURREALDB_URL": self.surrealdb_url,
                "SURREALDB_NS": self.surrealdb_ns,
                "SURREALDB_DB": self.surrealdb_db,
                "SURREALDB_USER": self.surrealdb_user,
                "SURREALDB_PASS": self.surrealdb_pass,
            }
        )
        env.setdefault("RUST_LOG", "warn")
        return env


def _read_required_env(variable_name: str) -> str:
    value = os.getenv(variable_name)
    if not value:
        raise RuntimeError(f"Missing required environment variable: {variable_name}. Set it in {DOTENV_PATH}.")
    return value


def _resolve_server_process() -> ServerProcessConfig:
    command_override = os.getenv("SURREALMCP_SERVER_COMMAND")
    if command_override:
        parts = shlex.split(command_override)
        if not parts:
            raise RuntimeError("SURREALMCP_SERVER_COMMAND is empty.")
        return ServerProcessConfig(command=parts[0], args=parts[1:], cwd=REPO_ROOT)

    debug_binary = REPO_ROOT / "target" / "debug" / "surrealmcp"
    if debug_binary.exists():
        return ServerProcessConfig(command=str(debug_binary), args=["start"], cwd=REPO_ROOT)

    return ServerProcessConfig(
        command="cargo",
        args=["run", "--quiet", "--manifest-path", str(REPO_ROOT / "Cargo.toml"), "--", "start"],
        cwd=REPO_ROOT,
    )


def load_settings() -> SmokeSettings:
    return SmokeSettings(
        surrealdb_url=_read_required_env("SURREALDB_URL"),
        surrealdb_ns=_read_required_env("SURREALDB_NS"),
        surrealdb_db=_read_required_env("SURREALDB_DB"),
        surrealdb_user=_read_required_env("SURREALDB_USER"),
        surrealdb_pass=_read_required_env("SURREALDB_PASS"),
        server=_resolve_server_process(),
    )
