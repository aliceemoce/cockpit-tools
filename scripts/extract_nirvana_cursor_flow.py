#!/usr/bin/env python3
"""从 nirvana 主 bundle 提取 Cursor 切号函数片段。"""
from __future__ import annotations

import json
import re
from pathlib import Path

BUNDLE = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\scripts\nirvana_extracted\dist-electron\main-FXxcqbQA.js")
OUT = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\scripts\nirvana_cursor_flow.json")

MARKERS = [
    "switchAccount",
    "patchCursorWorkbench",
    "patchCursorMachineId",
    "resetMachineIdFile",
    "resetStorageJsonIds",
    "resetWindowsMachineGuid",
    "FingerprintReset",
    "taskkill /IM Cursor.exe",
    "cursorAuth/cachedEmail",
    "storeAccessRefreshToken",
    "injectCursor",
    "switchCursor",
    "launchCursor",
    "startCursor",
]


def snippet(text: str, needle: str, radius: int = 400) -> str:
    i = text.find(needle)
    if i < 0:
        i = text.lower().find(needle.lower())
    if i < 0:
        return ""
    return re.sub(r"\s+", " ", text[max(0, i - radius) : i + radius])


def main() -> None:
    text = BUNDLE.read_text(encoding="utf-8", errors="replace")
    flows = []
    for m in MARKERS:
        s = snippet(text, m)
        if s:
            flows.append({"marker": m, "snippet": s[:800]})
    report = {
        "bundle": str(BUNDLE),
        "size": len(text),
        "flows": flows,
        "ordered_steps_traditional_switch": [
            "1. closeCursor (taskkill Cursor.exe)",
            "2. switchTokensInDb (delete old cursorAuth, reset vscdb telemetry, write tokens)",
            "3. resetStorageJsonIds (independent telemetry in storage.json)",
            "4. resetMachineIdFile (independent UUID in machineId file)",
            "5. patchCursorMachineId (main.js, NOT workbench)",
            "6. resetWindowsMachineGuid (Windows, optional on failure)",
            "7. sleep 1500ms",
            "8. startCursor",
        ],
        "not_in_traditional_switch": [
            "cleanCursorEnvironment (Vh) — cleaner UI only",
            "patchCursorWorkbench (Xh) — seamless setup only",
            "silentSwitch (Zh) — password login path only",
        ],
    }
    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print("written", OUT)


if __name__ == "__main__":
    main()
