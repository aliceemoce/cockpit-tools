#!/usr/bin/env python3
"""Audit all provider account indexes: live vs credentials mirror."""
from __future__ import annotations

import json
import os
from pathlib import Path

LIVE = Path(os.environ.get("COCKPIT_DATA_DIR", Path.home() / ".antigravity_cockpit"))
MIRROR = Path(
    os.environ.get(
        "COCKPIT_CREDENTIALS_DIR",
        Path.home() / "dev" / "cockpit-credentials",
    )
)

PROVIDERS = [
    ("antigravity", "accounts.json", "accounts"),
    ("codex", "codex_accounts.json", "codex_accounts"),
    ("github_copilot", "github_copilot_accounts.json", "github_copilot_accounts"),
    ("windsurf", "windsurf_accounts.json", "windsurf_accounts"),
    ("kiro", "kiro_accounts.json", "kiro_accounts"),
    ("cursor", "cursor_accounts.json", "cursor_accounts"),
    ("gemini", "gemini_accounts.json", "gemini_accounts"),
    ("codebuddy", "codebuddy_accounts.json", "codebuddy_accounts"),
    ("codebuddy_cn", "codebuddy_cn_accounts.json", "codebuddy_cn_accounts"),
    ("qoder", "qoder_accounts.json", "qoder_accounts"),
    ("trae", "trae_accounts.json", "trae_accounts"),
    ("workbuddy", "workbuddy_accounts.json", "workbuddy_accounts"),
    ("zed", "zed_accounts.json", "zed_accounts"),
]

OUT = Path(__file__).resolve().parent / "audit_all_accounts_report.json"


def count_index(path: Path) -> int | None:
    if not path.is_file():
        return None
    data = json.loads(path.read_text(encoding="utf-8"))
    accounts = data.get("accounts")
    if isinstance(accounts, list):
        return len(accounts)
    return None


def count_detail_dir(base: Path, sub: str) -> int:
    for candidate in [base / "data" / sub, base / sub]:
        if candidate.is_dir():
            return len(list(candidate.glob("*.json")))
    return 0


def main() -> None:
    rows: list[dict] = []
    total_live_index = 0
    total_mirror_index = 0

    for name, index_file, detail_sub in PROVIDERS:
        live_index = count_index(LIVE / index_file)
        mirror_index = count_index(MIRROR / index_file)
        if mirror_index is None:
            mirror_index = count_index(MIRROR / "data" / index_file)
        live_detail = count_detail_dir(LIVE, detail_sub)
        mirror_detail = count_detail_dir(MIRROR, detail_sub)
        row = {
            "provider": name,
            "live_index": live_index,
            "mirror_index": mirror_index,
            "live_detail_files": live_detail,
            "mirror_detail_files": mirror_detail,
            "index_delta_mirror_minus_live": (
                (mirror_index or 0) - (live_index or 0)
                if live_index is not None and mirror_index is not None
                else None
            ),
        }
        rows.append(row)
        total_live_index += live_index or 0
        total_mirror_index += mirror_index or 0

    report = {
        "live_base": str(LIVE),
        "mirror_base": str(MIRROR),
        "providers": rows,
        "total_live_index": total_live_index,
        "total_mirror_index": total_mirror_index,
        "total_index_delta": total_mirror_index - total_live_index,
    }
    OUT.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
