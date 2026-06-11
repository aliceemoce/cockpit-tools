#!/usr/bin/env python3
"""静态验证：无忧 i() 链是否逐字复制到 Rust（Kh + go + 1500ms）。"""

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

    nirvana_symbols = {
        "switch_tokens_nirvana_kh": cursor_account,
        "nirvana_traditional_switch_steps": cursor_account,
        "nirvana_traditional_switch_and_start": cursor_account,
        "close_cursor_nirvana_style": cursor_instance,
        "start_cursor_nirvana_go": cursor_instance,
        "explorer.exe": cursor_instance,
        'cachedSignUpType", "Auth_0"': cursor_account,
    }
    for name, blob in nirvana_symbols.items():
        if name not in blob:
            errors.append(f"缺少无忧复制符号: {name}")

    if "resolve_vscdb_auth_tokens" in cursor_account.split("switch_tokens_nirvana_kh", 1)[-1].split("pub fn switch_tokens_in_profile_db", 1)[0]:
        errors.append("switch_tokens_nirvana_kh 仍调用 resolve_vscdb_auth_tokens（应直接写原始 token）")

    if "nirvana_traditional_switch_and_start" not in cursor_cmd:
        errors.append("Play/inject_cursor_account 未走 nirvana_traditional_switch_and_start")

    if "start_cursor_default_with_args_with_new_window" in cursor_instance_cmd.split("skip_prelaunch_close", 1)[-1][:800]:
        errors.append("默认实例启动仍用 --user-data-dir，未改 explorer.exe go()")

    if re.search(r"多开实例跳过 MachineGuid", switch_align):
        errors.append("仍存在「多开跳过 MachineGuid」")

    if "sleep(std::time::Duration::from_millis(1500))" not in cursor_account:
        errors.append("nirvana_traditional_switch_and_start 缺少 1500ms")

    report = {
        "ok": len(errors) == 0,
        "errors": errors,
        "flow_matches_nirvana": len(errors) == 0,
        "nirvana_i_steps": [
            "closeCursor",
            "switchTokensInDb(Kh)",
            "resetStorageJsonIds(Gh)",
            "resetMachineIdFile(Jh)",
            "patchCursorMachineId(Yh)",
            "resetWindowsMachineGuid(Nc)",
            "sleep 1500ms",
            "startCursor(go)=explorer.exe",
        ],
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    if errors:
        print("FAIL:")
        for err in errors:
            print(f"  - {err}")
        return 1
    print("PASS: 无忧 i() + go() 已复制到 Rust")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
