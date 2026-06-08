#!/usr/bin/env python3
"""Trigger list_cursor_accounts by launching a one-shot check via cargo test hook."""
from __future__ import annotations

import json
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = Path(__file__).resolve().parent / "verify_cursor_list_report.json"


def main() -> int:
    env = os.environ.copy()
    env.setdefault("COCKPIT_CREDENTIALS_DIR", str(Path.home() / "dev" / "cockpit-credentials"))
    cmd = [
        "cargo",
        "test",
        "-p",
        "cockpit-tools",
        "cursor_account_mirror_restore_counts_index_entries",
        "--",
        "--nocapture",
    ]
    proc = subprocess.run(
        cmd,
        cwd=ROOT / "src-tauri",
        env=env,
        capture_output=True,
        text=True,
        timeout=600,
    )
    report = {
        "exit_code": proc.returncode,
        "stdout_tail": proc.stdout[-4000:],
        "stderr_tail": proc.stderr[-4000:],
    }
    OUT.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))
    return proc.returncode


if __name__ == "__main__":
    sys.exit(main())
