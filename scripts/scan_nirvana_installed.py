#!/usr/bin/env python3
"""扫描已安装 nirvana (无忧小助手) 解压目录，输出 Cursor 切号逻辑片段。"""
from __future__ import annotations

import json
import re
from pathlib import Path

ROOT = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\scripts\nirvana_extracted")
FALLBACK_ASAR = Path(r"C:\Program Files\nirvana\resources\app.asar")
OUT = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\scripts\nirvana_cursor_logic_report.json")

KEYS = [
    "state.vscdb",
    "storage.json",
    "machineId",
    "cursorAuth",
    "telemetry.machineId",
    "telemetry.macMachineId",
    "telemetry.devDeviceId",
    "storage.serviceMachineId",
    "globalStorage",
    "cursor.exe",
    "taskkill",
    "user-data-dir",
    "accessToken",
    "refreshToken",
    "switchCursor",
    "switchAccount",
    "injectCursor",
    "resetMachine",
    "resetFingerprint",
]


def main() -> None:
    if not ROOT.is_dir() and FALLBACK_ASAR.is_file():
        import subprocess
        ROOT.mkdir(parents=True, exist_ok=True)
        subprocess.run(
            ["npx", "--yes", "@electron/asar", "extract", str(FALLBACK_ASAR), str(ROOT)],
            check=False,
        )

    report = {
        "root": str(ROOT),
        "exists": ROOT.is_dir(),
        "package": None,
        "files_scanned": 0,
        "matches": [],
        "inferred_steps": [],
    }
    pkg = ROOT / "package.json"
    if pkg.is_file():
        report["package"] = json.loads(pkg.read_text(encoding="utf-8"))

    for path in ROOT.rglob("*.js"):
        if path.stat().st_size > 8_000_000:
            continue
        report["files_scanned"] += 1
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except Exception:
            continue
        rel = str(path.relative_to(ROOT))
        for key in KEYS:
            if key.lower() in text.lower():
                # extract tight context
                idx = text.lower().find(key.lower())
                start = max(0, idx - 120)
                end = min(len(text), idx + 200)
                snippet = re.sub(r"\s+", " ", text[start:end])
                report["matches"].append({"file": rel, "key": key, "snippet": snippet})
                if len(report["matches"]) >= 120:
                    break
        if len(report["matches"]) >= 120:
            break

    joined = json.dumps(report["matches"], ensure_ascii=False).lower()
    if "taskkill" in joined or "cursor.exe" in joined:
        report["inferred_steps"].append("terminate_cursor_process")
    if "telemetry" in joined or "machineid" in joined:
        report["inferred_steps"].append("reset_telemetry_ids_storage_json_and_vscdb")
    if "cursorauth" in joined or "accesstoken" in joined:
        report["inferred_steps"].append("write_cursorAuth_to_state_vscdb")
    if "user-data-dir" in joined:
        report["inferred_steps"].append("launch_cursor_with_user_data_dir")

    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
