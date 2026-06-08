#!/usr/bin/env python3
"""解压 nirvana app.asar 并提取 Cursor 切号相关 JS 逻辑片段（外部程序，非 cockpit 模块）。"""
from __future__ import annotations

import json
import re
import shutil
import subprocess
import sys
from pathlib import Path

NIRVANA = Path(r"C:\Program Files\nirvana")
ASAR = NIRVANA / "resources" / "app.asar"
OUT_DIR = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\scripts\nirvana_extracted")
REPORT = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\scripts\nirvana_cursor_logic_report.json")

PATTERNS = [
    r"state\.vscdb",
    r"storage\.json",
    r"machineId",
    r"cursorAuth",
    r"telemetry\.machineId",
    r"telemetry\.macMachineId",
    r"telemetry\.devDeviceId",
    r"globalStorage",
    r"accessToken",
    r"user-data-dir",
    r"Cursor",
    r"switchAccount",
    r"switch.*account",
    r"fingerprint",
    r"kill.*cursor",
    r"close.*cursor",
]


def run(cmd: list[str], cwd: Path | None = None) -> tuple[int, str]:
    p = subprocess.run(cmd, cwd=cwd, capture_output=True, text=True, encoding="utf-8", errors="replace")
    out = (p.stdout or "") + (p.stderr or "")
    return p.returncode, out


def extract_asar() -> bool:
    if OUT_DIR.exists():
        shutil.rmtree(OUT_DIR, ignore_errors=True)
    OUT_DIR.mkdir(parents=True, exist_ok=True)
    if not ASAR.is_file():
        return False
    code, out = run(["npx", "--yes", "@electron/asar", "extract", str(ASAR), str(OUT_DIR)])
    if code != 0:
        # fallback older package name
        code2, out2 = run(["npx", "--yes", "asar", "extract", str(ASAR), str(OUT_DIR)])
        if code2 != 0:
            raise RuntimeError(f"asar extract failed: {out}\n{out2}")
    return True


def scan_js_files() -> list[dict]:
    hits: list[dict] = []
    for path in OUT_DIR.rglob("*"):
        if path.suffix.lower() not in {".js", ".mjs", ".cjs", ".ts"}:
            continue
        try:
            text = path.read_text(encoding="utf-8", errors="replace")
        except Exception:
            continue
        rel = str(path.relative_to(OUT_DIR))
        for pat in PATTERNS:
            if re.search(pat, text, re.I):
                # grab context lines
                lines = []
                for i, line in enumerate(text.splitlines(), 1):
                    if re.search(pat, line, re.I):
                        lines.append({"line": i, "text": line.strip()[:240]})
                        if len(lines) >= 8:
                            break
                hits.append({"file": rel, "pattern": pat, "samples": lines})
    return hits


def infer_flow(hits: list[dict]) -> list[str]:
    joined = json.dumps(hits, ensure_ascii=False).lower()
    steps: list[str] = []
    if "kill" in joined or "close" in joined or "taskkill" in joined:
        steps.append("1. 关闭 Cursor 进程")
    if "machineid" in joined or "telemetry" in joined or "fingerprint" in joined:
        steps.append("2. 重置/改写 machineId 与 telemetry 字段")
    if "storage.json" in joined:
        steps.append("3. 修改 storage.json")
    if "state.vscdb" in joined or "cursorauth" in joined:
        steps.append("4. 写入 state.vscdb cursorAuth/* token")
    if "user-data-dir" in joined or "spawn" in joined or "exec" in joined:
        steps.append("5. 启动 Cursor")
    return steps or ["未能从 JS 推断完整流程（可能混淆或动态加载）"]


def main() -> int:
    report: dict = {"asar": str(ASAR), "asar_exists": ASAR.is_file(), "extracted": False, "hits": [], "inferred_flow": []}
    if not ASAR.is_file():
        alt = list((NIRVANA / "resources").glob("**/*.asar")) if (NIRVANA / "resources").is_dir() else []
        report["asar_alternatives"] = [str(p) for p in alt]
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 1

    report["extracted"] = extract_asar()
    report["file_count"] = sum(1 for _ in OUT_DIR.rglob("*") if _.is_file())
    hits = scan_js_files()
    report["hits"] = hits[:200]
    report["hit_count"] = len(hits)
    report["inferred_flow"] = infer_flow(hits)
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0


if __name__ == "__main__":
    sys.exit(main())
