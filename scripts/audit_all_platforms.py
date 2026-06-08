#!/usr/bin/env python3
"""Audit account index vs detail files for ALL platforms + credentials mirror."""
from __future__ import annotations

import json
import os
from pathlib import Path

BASE = Path(os.environ.get("COCKPIT_DATA_DIR", Path.home() / ".antigravity_cockpit"))
CRED = Path(os.environ.get("COCKPIT_CREDENTIALS_DIR", Path.home() / "dev" / "cockpit-credentials"))
OUT = Path(__file__).resolve().parent / "audit_all_platforms_report.json"

PLATFORMS = [
    ("antigravity", "accounts.json", "accounts"),
    ("cursor", "cursor_accounts.json", "cursor_accounts"),
    ("codex", "codex_accounts.json", "codex_accounts"),
    ("windsurf", "windsurf_accounts.json", "windsurf_accounts"),
    ("kiro", "kiro_accounts.json", "kiro_accounts"),
    ("gemini", "gemini_accounts.json", "gemini_accounts"),
    ("github_copilot", "github_copilot_accounts.json", "github_copilot_accounts"),
    ("codebuddy", "codebuddy_accounts.json", "codebuddy_accounts"),
    ("codebuddy_cn", "codebuddy_cn_accounts.json", "codebuddy_cn_accounts"),
    ("qoder", "qoder_accounts.json", "qoder_accounts"),
    ("trae", "trae_accounts.json", "trae_accounts"),
    ("workbuddy", "workbuddy_accounts.json", "workbuddy_accounts"),
    ("zed", "zed_accounts.json", "zed_accounts"),
]


def count_index(index_path: Path) -> int:
    if not index_path.is_file():
        return 0
    data = json.loads(index_path.read_text(encoding="utf-8"))
    accounts = data.get("accounts") or []
    return len(accounts)


def count_detail_dir(detail_dir: Path) -> int:
    if not detail_dir.is_dir():
        return 0
    return len(list(detail_dir.glob("*.json")))


def audit_root(root: Path, label: str) -> dict:
    rows: list[dict] = []
    total_index = 0
    total_files = 0
    for _key, index_name, detail_name in PLATFORMS:
        index_path = root / index_name
        detail_dir = root / detail_name
        if label == "credentials" and (root / "data" / detail_name).is_dir():
            detail_dir = root / "data" / detail_name
        ic = count_index(index_path)
        fc = count_detail_dir(detail_dir)
        total_index += ic
        total_files += fc
        rows.append(
            {
                "platform": _key,
                "index_count": ic,
                "file_count": fc,
                "missing_detail": max(0, ic - fc),
                "orphan_files": max(0, fc - ic),
            }
        )
    return {
        "label": label,
        "root": str(root),
        "total_index": total_index,
        "total_detail_files": total_files,
        "platforms": rows,
    }


def main() -> None:
    report = {
        "live": audit_root(BASE, "live"),
        "credentials": audit_root(CRED, "credentials"),
    }
    # cursor email orphans: files not in index ids
    cursor_index = BASE / "cursor_accounts.json"
    cursor_dir = BASE / "cursor_accounts"
    orphan_emails_note = None
    if cursor_index.is_file() and cursor_dir.is_dir():
        idx = json.loads(cursor_index.read_text(encoding="utf-8"))
        index_ids = {s["id"] for s in idx.get("accounts", []) if s.get("id")}
        file_ids = {p.stem for p in cursor_dir.glob("*.json")}
        report["cursor_id_orphans"] = sorted(file_ids - index_ids)[:20]
        report["cursor_id_orphan_count"] = len(file_ids - index_ids)
        report["cursor_index_ids"] = len(index_ids)
        report["cursor_file_ids"] = len(file_ids)
    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
