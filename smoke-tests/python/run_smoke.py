"""Run the local Python smoke test suite with dotenv configuration loaded."""

from __future__ import annotations

import os
import subprocess
import sys
from pathlib import Path

from dotenv import load_dotenv


def main(args: list[str]) -> int:
    """Execute pytest in this workspace and return pytest's exit code."""
    workspace_root = Path(__file__).resolve().parent
    load_dotenv(workspace_root / ".env", override=False)

    command = [sys.executable, "-m", "pytest", "-q", *args]
    completed = subprocess.run(command, cwd=workspace_root, env=os.environ.copy(), check=False)
    return completed.returncode


if __name__ == "__main__":
    raise SystemExit(main(sys.argv[1:]))
