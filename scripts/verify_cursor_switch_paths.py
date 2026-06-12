#!/usr/bin/env python3
"""静态验证：无忧 i() 切号链 + token session 格式 + 10min/分批刷新。"""

from __future__ import annotations

import json
import re
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUST = ROOT / "src-tauri" / "src"
REPORT = ROOT / "scripts" / "backend_verify_report.json"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def main() -> int:
    errors: list[str] = []
    cursor_account = read(RUST / "modules" / "cursor_account.rs")
    cursor_instance = read(RUST / "modules" / "cursor_instance.rs")
    cursor_instance_cmd = read(RUST / "commands" / "cursor_instance.rs")
    cursor_cmd = read(RUST / "commands" / "cursor.rs")
    switch_align = read(RUST / "modules" / "cursor_switch_align.rs")
    scheduler = read(RUST / "modules" / "cursor_refresh_scheduler.rs")
    defer = read(RUST / "modules" / "app_startup_defer.rs")

    required = [
        ("close_cursor_nirvana_style", cursor_instance),
        ("switch_tokens_in_profile_db", cursor_account),
        ("reset_storage_json_ids_for_profile", cursor_account),
        ("reset_machine_id_file_for_profile", cursor_account),
        ("apply_nirvana_traditional_switch_patches", cursor_account),
        ("resolve_vscdb_auth_tokens", cursor_account),
        ("start_cursor_instance_with_account_switch", cursor_instance_cmd),
        ("sleep(std::time::Duration::from_millis(1500))", cursor_instance_cmd),
        ("refresh_all_tokens_batched", cursor_account),
        ("CURRENT_QUOTA_REFRESH_SECS: u64 = 20", scheduler),
        ("STARTUP_DEFER_SECS: u64 = 10 * 60", defer),
    ]
    for needle, blob in required:
        if needle not in blob:
            errors.append(f"缺少: {needle}")

    if "start_cursor_instance_with_account_switch" not in cursor_cmd:
        errors.append("Play 未走 start_cursor_instance_with_account_switch")

    if re.search(r"多开实例跳过 MachineGuid", switch_align):
        errors.append("仍存在「多开跳过 MachineGuid」")

    if "pull_remote" in cursor_account:
        errors.append("仍含 pull_remote")

    if "unwrap_or(&account.access_token)" in cursor_account.split("switch_tokens_in_profile_db")[1][:1200]:
        errors.append("switch_tokens 仍用 access_token 顶替 refresh（会导致登录页）")

    report = {
        "ok": len(errors) == 0,
        "errors": errors,
        "flow_matches_nirvana": len(errors) == 0,
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    if errors:
        print("FAIL:")
        for err in errors:
            print(f"  - {err}")
        return 1
    print("PASS: 无忧切号链 + session token + 10min + 20s + 分批")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
