#!/usr/bin/env python3
"""
三方对比（无「小助手模块」）：
1. 主仓库 jlcodes99/cockpit-tools main — 远程拉取
2. 当前 fork 本地源码
3. 桌面独立程序 nirvana.exe — 仅字符串扫描（不在 cockpit 仓库内）

输出: scripts/three_way_compare_report.json
"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

REPO = Path(r"C:\Users\aliceemoce\dev\cockpit-tools")
OUT = REPO / "scripts" / "three_way_compare_report.json"
UPSTREAM = "https://raw.githubusercontent.com/jlcodes99/cockpit-tools/main"
NIRVANA = Path(r"C:\Program Files\nirvana\nirvana.exe")

RUST_FILES = [
    "src-tauri/src/commands/cursor.rs",
    "src-tauri/src/commands/cursor_instance.rs",
    "src-tauri/src/modules/cursor_account.rs",
]

CURSOR_MARKERS = [
    "inject_cursor_account",
    "inject_to_cursor",
    "switch_cursor_account_to_profile",
    "start_cursor_instance_with_account_switch",
    "inject_bound_account_for_instance_start",
    "hard_reset_cursor_fingerprint",
]

NIRVANA_MARKERS = [
    b"state.vscdb",
    b"storage.json",
    b"machineId",
    b"cursorAuth",
    b"telemetry.machineId",
    b"globalStorage",
]


def fetch(url: str) -> str:
    with urllib.request.urlopen(url, timeout=90) as r:
        return r.read().decode("utf-8", errors="replace")


def scan_rust(text: str) -> dict[str, bool]:
    return {m: m in text for m in CURSOR_MARKERS}


def scan_nirvana_exe() -> dict:
    if not NIRVANA.is_file():
        return {"exists": False, "error": "nirvana.exe not found"}
    data = NIRVANA.read_bytes()
    hits = {m.decode("ascii", errors="ignore"): (m in data) for m in NIRVANA_MARKERS}
    return {
        "exists": True,
        "path": str(NIRVANA),
        "size": NIRVANA.stat().st_size,
        "string_hits": hits,
        "note": "独立程序；非 cockpit-tools 源码。仅证明其操作 Cursor 数据文件。",
    }


def summarize_upstream(files: dict[str, str]) -> list[str]:
    c = files.get("src-tauri/src/commands/cursor.rs", "")
    i = files.get("src-tauri/src/commands/cursor_instance.rs", "")
    steps = ["inject_cursor_account -> inject_to_cursor"]
    if "cursor_start_instance" in c:
        steps.append("-> cursor_start_instance(__default__)")
    if "inject_bound_account_for_instance_start" in i:
        steps.append("instance: inject_bound only if bind_account_id set")
    if "hard_reset_cursor_fingerprint" not in "".join(files.values()):
        steps.append("无 hard_reset_cursor_fingerprint")
    return steps


def summarize_local(files: dict[str, str]) -> list[str]:
    joined = "".join(files.values())
    if "start_cursor_instance_with_account_switch" in joined:
        return [
            "inject_cursor_account -> start_cursor_instance_with_account_switch(__default__, Some(id))",
            "cursor_start_instance -> start_cursor_instance_with_account_switch(id, None)",
            "-> switch_cursor_account_to_profile (close+reset+inject)",
            "-> cursor_start_instance_prepared (launch only)",
        ]
    return ["未检测到统一入口"]


def main() -> int:
    report: dict = {}

    upstream_files: dict[str, str] = {}
    for rel in RUST_FILES:
        try:
            upstream_files[rel] = fetch(f"{UPSTREAM}/{rel}")
        except Exception as e:
            upstream_files[rel] = f"/* fetch error: {e} */"

    local_files = {rel: (REPO / rel).read_text(encoding="utf-8", errors="replace") for rel in RUST_FILES}

    report["cockpit_has_assistant_strings"] = any(
        re.search(r"无忧|nirvana|jzzcg", text, re.I) for text in local_files.values()
    )
    report["upstream"] = {rel: scan_rust(t) for rel, t in upstream_files.items() if not t.startswith("/*")}
    report["local"] = {rel: scan_rust(t) for rel, t in local_files.items()}
    report["upstream_flow"] = summarize_upstream(upstream_files)
    report["local_flow"] = summarize_local(local_files)
    report["nirvana_external"] = scan_nirvana_exe()
    report["local_unified"] = all(
        local_files["src-tauri/src/commands/cursor_instance.rs"].count(m) > 0
        for m in (
            "start_cursor_instance_with_account_switch",
            "switch_cursor_account_to_profile",
        )
    ) and "inject_bound_account_for_instance_start" not in local_files[
        "src-tauri/src/commands/cursor_instance.rs"
    ]

    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
