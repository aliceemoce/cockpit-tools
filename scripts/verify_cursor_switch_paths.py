#!/usr/bin/env python3
"""静态验证：upstream 串行全量 refresh，无 fork 分批/调度器。"""

from __future__ import annotations

import json
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
RUST = ROOT / "src-tauri" / "src"
REPORT = ROOT / "scripts" / "backend_verify_report.json"


def read(path: Path) -> str:
    return path.read_text(encoding="utf-8", errors="replace")


def main() -> int:
    errors: list[str] = []
    warnings: list[str] = []
    cursor_account = read(RUST / "modules" / "cursor_account.rs")
    lib_rs = read(RUST / "lib.rs")

    required = [
        ("pub async fn refresh_all_tokens", cursor_account),
        ("refresh_account_async(&id).await", cursor_account),
    ]
    for needle, blob in required:
        if needle not in blob:
            errors.append(f"缺少: {needle}")

    forbidden = [
        ("refresh_all_tokens_batched", cursor_account),
        ("分批次刷新", cursor_account),
        ("cursor_refresh_scheduler", lib_rs),
        ("app_startup_defer", lib_rs),
    ]
    for needle, blob in forbidden:
        if needle in blob:
            errors.append(f"仍含 fork 增量: {needle}")

    if (RUST / "modules" / "cursor_refresh_scheduler.rs").exists():
        errors.append("cursor_refresh_scheduler.rs 仍存在")
    if (RUST / "modules" / "app_startup_defer.rs").exists():
        errors.append("app_startup_defer.rs 仍存在")

    # 默认 24 小时（cockpit-core + src-tauri config）
    for cfg_path in (
        ROOT / "crates" / "cockpit-core" / "src" / "modules" / "config.rs",
        ROOT / "src-tauri" / "src" / "modules" / "config.rs",
    ):
        cfg = read(cfg_path)
        if "fn default_cursor_auto_refresh()" not in cfg or "\n    1440\n" not in cfg.split(
            "fn default_cursor_auto_refresh()", 1
        )[-1][:80]:
            errors.append(f"{cfg_path.name} 中 default_cursor_auto_refresh 非 1440")

    report = {
        "ok": len(errors) == 0,
        "errors": errors,
        "warnings": warnings,
        "fork_refresh_reverted": len(errors) == 0,
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    if errors:
        print("FAIL:")
        for err in errors:
            print(f"  - {err}")
        return 1
    print("PASS: upstream 串行 refresh，无 fork 分批/调度器，Cursor 默认 24 小时")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
