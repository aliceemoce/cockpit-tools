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
        "ordered_steps_inferred": [
            "1. taskkill Cursor.exe",
            "2. resetMachineIdFile (machineid file)",
            "3. resetStorageJsonIds (telemetry.* in storage.json)",
            "4. reset state.vscdb ItemTable telemetry + storage.serviceMachineId",
            "5. optional resetWindowsMachineGuid",
            "6. patch Cursor workbench main.js -> __cursorAuthBridge.switchAccount(access,refresh,email,signUpType)",
            "7. launch Cursor",
        ],
    }
    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print("written", OUT)


if __name__ == "__main__":
    main()
