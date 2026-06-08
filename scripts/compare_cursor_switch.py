#!/usr/bin/env python3
"""对比主仓库 / fork 的 Cursor 切号入口与调用链，输出 JSON 报告。"""
from __future__ import annotations

import json
import re
import subprocess
import sys
import urllib.request
from pathlib import Path

REPO = Path(r"C:\Users\aliceemoce\dev\cockpit-tools")
OUT = REPO / "scripts" / "cursor_switch_compare_report.json"
UPSTREAM = "https://raw.githubusercontent.com/jlcodes99/cockpit-tools/main"

FILES = [
    "src-tauri/src/commands/cursor.rs",
    "src-tauri/src/commands/cursor_instance.rs",
    "src-tauri/src/modules/cursor_account.rs",
]

MARKERS = [
    "inject_cursor_account",
    "inject_to_cursor",
    "switch_cursor_account_to_profile",
    "hard_reset_cursor_fingerprint",
    "start_cursor_instance_with_account_switch",
    "cursor_start_instance_prepared",
    "inject_bound_account_for_instance_start",
    "inject_account_to_profile",
]


def fetch_upstream(rel: str) -> str:
    url = f"{UPSTREAM}/{rel}"
    with urllib.request.urlopen(url, timeout=60) as resp:
        return resp.read().decode("utf-8", errors="replace")


def scan(text: str) -> dict[str, list[int]]:
    hits: dict[str, list[int]] = {}
    for m in MARKERS:
        lines = [i + 1 for i, line in enumerate(text.splitlines()) if m in line]
        if lines:
            hits[m] = lines
    return hits


def local_text(rel: str) -> str:
    return (REPO / rel).read_text(encoding="utf-8", errors="replace")


def flow_summary(hits: dict[str, list[int]], label: str) -> list[str]:
    steps: list[str] = []
    if "start_cursor_instance_with_account_switch" in hits:
        steps.append(f"{label}: unified start_cursor_instance_with_account_switch")
        if "switch_cursor_account_to_profile" in hits:
            steps.append("  -> switch_cursor_account_to_profile (close+reset+inject)")
        if "cursor_start_instance_prepared" in hits:
            steps.append("  -> cursor_start_instance_prepared (launch only)")
        return steps

    if "inject_cursor_account" in hits:
        steps.append(f"{label} inject_cursor_account:")
        if "inject_to_cursor" in hits and "switch_cursor_account_to_profile" not in hits:
            steps.append("  -> inject_to_cursor (no unified switch)")
        elif "switch_cursor_account_to_profile" in hits or "hard_reset_cursor_fingerprint" in hits:
            steps.append("  -> hard_reset + inject (partial/legacy path)")
        if "cursor_start_instance" in hits.get("inject_cursor_account", []) or True:
            if "inject_bound_account_for_instance_start" in hits:
                steps.append("  -> cursor_start_instance -> inject_bound (may double-inject)")

    if "inject_bound_account_for_instance_start" in hits:
        steps.append(f"{label} instance start: inject_bound only when bind set")
    return steps


def main() -> int:
    report: dict = {"upstream": {}, "local": {}, "flows": {}, "consistent_local": False}

    for rel in FILES:
        try:
            up = fetch_upstream(rel)
            report["upstream"][rel] = scan(up)
        except Exception as e:
            report["upstream"][rel] = {"error": str(e)}
        report["local"][rel] = scan(local_text(rel))

    local_hits = {}
    for v in report["local"].values():
        if isinstance(v, dict) and "error" not in v:
            for k, lines in v.items():
                local_hits.setdefault(k, []).extend(lines)

    up_hits = {}
    for v in report["upstream"].values():
        if isinstance(v, dict) and "error" not in v:
            for k, lines in v.items():
                up_hits.setdefault(k, []).extend(lines)

    report["flows"]["upstream"] = flow_summary(up_hits, "upstream")
    report["flows"]["local"] = flow_summary(local_hits, "local")

    unified = (
        "start_cursor_instance_with_account_switch" in local_hits
        and "switch_cursor_account_to_profile" in local_hits
        and "inject_bound_account_for_instance_start" not in local_hits
    )
    inject_uses_unified = False
    cursor_rs = local_text(FILES[0])
    if re.search(
        r"start_cursor_instance_with_account_switch\s*\(\s*\"__default__\"",
        cursor_rs,
    ):
        inject_uses_unified = True

    report["consistent_local"] = unified and inject_uses_unified
    report["fork_has_wuyou_strings"] = any(
        "无忧" in local_text(f) or "nirvana" in local_text(f).lower()
        for f in FILES
    )

    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["consistent_local"] and not report["fork_has_wuyou_strings"] else 1


if __name__ == "__main__":
    sys.exit(main())
