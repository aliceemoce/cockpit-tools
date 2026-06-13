#!/usr/bin/env python3
"""静态验证：0.24.8 bridge 切号链 + upstream 全量 refresh + 10min/分批刷新。"""

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
        ("apply_pre_inject_cursor_patches", switch_align),
        ("hard_reset_cursor_fingerprint_state_for_profile", cursor_account),
        ("clear_switch_auth_keys_for_profile", cursor_account),
        ("inject_account_to_profile", cursor_instance),
        ("ensure_state_db_for_injection", cursor_instance),
        ("start_cursor_instance_with_account_switch", cursor_instance_cmd),
        ("refresh_all_tokens_batched", cursor_account),
        ("mode=full", cursor_account),
        ("CURRENT_QUOTA_REFRESH_SECS: u64 = 20", scheduler),
        ("STARTUP_DEFER_SECS: u64 = 10 * 60", defer),
    ]
    for needle, blob in required:
        if needle not in blob:
            errors.append(f"缺少: {needle}")

    if "start_cursor_instance_with_account_switch" not in cursor_cmd:
        errors.append("Play 未走 start_cursor_instance_with_account_switch")

    if "nirvana_traditional_switch_steps" in cursor_account:
        errors.append("仍含 nirvana_traditional_switch_steps")

    if "apply_nirvana_traditional_switch_patches" in switch_align:
        errors.append("仍含 apply_nirvana_traditional_switch_patches")

    if "refresh_account_quota_only_async" in cursor_account:
        errors.append("仍含 refresh_account_quota_only_async")

    switch_body = cursor_account.split("pub fn switch_cursor_account_to_profile", 1)
    if len(switch_body) > 1 and "apply_pre_inject_cursor_patches" not in switch_body[1][:1500]:
        errors.append("switch_cursor_account_to_profile 未走 bridge 链")

    if "refresh_account_async(&id)" not in cursor_account and "refresh_account_async(&account_id)" not in cursor_account:
        if "refresh_account_async(&id)" not in cursor_account:
            errors.append("分批次刷新未调用 refresh_account_async")

    report = {
        "ok": len(errors) == 0,
        "errors": errors,
        "flow_matches_bridge": len(errors) == 0,
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    if errors:
        print("FAIL:")
        for err in errors:
            print(f"  - {err}")
        return 1
    print("PASS: bridge 切号链 + upstream refresh + 10min + 20s + 分批")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
