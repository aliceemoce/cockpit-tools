#!/usr/bin/env python3
"""Audit Cursor account index vs detail files; write report JSON."""
from __future__ import annotations

import json
import os
from pathlib import Path

BASE = Path(os.environ.get("COCKPIT_DATA_DIR", Path.home() / ".antigravity_cockpit"))
INDEX = BASE / "cursor_accounts.json"
DETAIL_DIR = BASE / "cursor_accounts"
BACKUP_DIR = BASE / "cursor_local_import_backups"
OUT = Path(__file__).resolve().parent / "audit_cursor_accounts_report.json"


def main() -> None:
    report: dict = {"base": str(BASE), "index_exists": INDEX.is_file()}
    if not INDEX.is_file():
        report["error"] = "cursor_accounts.json missing"
        OUT.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(json.dumps(report))
        return

    index = json.loads(INDEX.read_text(encoding="utf-8"))
    summaries = index.get("accounts") or []
    index_ids = {s["id"] for s in summaries if isinstance(s, dict) and s.get("id")}
    file_ids = set()
    null_usage = 0
    quota_errors = 0
    if DETAIL_DIR.is_dir():
        for path in DETAIL_DIR.glob("*.json"):
            file_ids.add(path.stem)
            try:
                acc = json.loads(path.read_text(encoding="utf-8"))
                if not acc.get("cursor_usage_raw"):
                    null_usage += 1
                if (acc.get("quota_query_last_error") or "").strip():
                    quota_errors += 1
            except Exception:
                pass

    missing_detail = sorted(index_ids - file_ids)
    orphan_files = sorted(file_ids - index_ids)
    backup_count = len(list(BACKUP_DIR.glob("*.json"))) if BACKUP_DIR.is_dir() else 0

    report.update(
        {
            "index_count": len(index_ids),
            "file_count": len(file_ids),
            "missing_detail_count": len(missing_detail),
            "orphan_file_count": len(orphan_files),
            "missing_detail_sample": missing_detail[:10],
            "null_usage_files": null_usage,
            "quota_error_files": quota_errors,
            "backup_count": backup_count,
        }
    )
    OUT.write_text(json.dumps(report, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
