#!/usr/bin/env python3
"""无鼠标：检查已部署 exe 与实例启动链日志关键字（不点换号，只看多开）。"""

from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
REPORT = ROOT / "scripts" / "instance_start_verify_report.json"
EXE = Path.home() / "AppData/Local/Cockpit Tools/cockpit-tools.exe"
LOG_DIR = Path.home() / ".antigravity_cockpit" / "logs"


def latest_log() -> Path | None:
    if not LOG_DIR.is_dir():
        return None
    files = sorted(LOG_DIR.glob("*.log"), key=lambda p: p.stat().st_mtime, reverse=True)
    return files[0] if files else None


def tail(path: Path, n: int = 80) -> str:
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        return "\n".join(lines[-n:])
    except OSError:
        return ""


def main() -> int:
    errors: list[str] = []
    notes: list[str] = []

    if not EXE.is_file():
        errors.append(f"未找到已安装 exe: {EXE}")
    else:
        notes.append(f"exe: {EXE} ({EXE.stat().st_size} bytes)")

    # 字符串探针：不应含已删逻辑
    if EXE.is_file():
        data = EXE.read_bytes()
        for needle in [b"restore_missing_detail_files_from_mirror", b"pull_remote_import", b"gh auth token"]:
            if needle in data:
                errors.append(f"exe 仍含禁止字符串: {needle.decode()}")

    log_path = latest_log()
    if log_path:
        notes.append(f"log: {log_path}")
        text = tail(log_path, 120)
        if "[Cursor Switch] 无忧传统路径换号完成" in text:
            notes.append("日志中曾出现无忧切号完成")
        if "switchTokensInDb 完成" in text:
            notes.append("日志中曾出现 switchTokensInDb")
        if "分批次刷新开始" in text:
            notes.append("日志中曾出现分批次刷新")
    else:
        notes.append("无应用日志—— 需启动 Cockpit 并点「多开实例 Start」后重跑本脚本")

    report = {"ok": len(errors) == 0, "errors": errors, "notes": notes}
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    if errors:
        print("FAIL:")
        for e in errors:
            print(f"  - {e}")
        return 1
    print("PASS (静态 exe 探针):")
    for n in notes:
        print(f"  {n}")
    if "需启动 Cockpit" in str(notes):
        print("WARN: 多开实例运行时验证尚未写入日志，请启动实例后重跑")
        return 2
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
