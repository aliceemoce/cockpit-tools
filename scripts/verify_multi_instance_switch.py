#!/usr/bin/env python3
"""多开实地验收：dual-cursor-launch + 日志 + profile auth 键核对。"""
from __future__ import annotations

import json
import re
import sqlite3
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

REPORT = Path(__file__).resolve().parent / "multi_instance_switch_report.json"
LOG_DIR = Path.home() / ".antigravity_cockpit" / "logs"
INSTANCES_JSON = Path.home() / ".antigravity_cockpit" / "cursor_instances.json"
DUAL_LAUNCH = Path(__file__).resolve().parents[1] / "target" / "release" / "dual-cursor-launch.exe"


def today_log() -> Path:
    day = datetime.now().strftime("%Y-%m-%d")
    primary = LOG_DIR / f"app.log.{day}"
    if primary.exists():
        return primary
    candidates = sorted(LOG_DIR.glob("app.log.*"), key=lambda p: p.stat().st_mtime, reverse=True)
    if candidates:
        return candidates[0]
    return primary


def first_instance_id() -> str:
    data = json.loads(INSTANCES_JSON.read_text(encoding="utf-8"))
    instances = data.get("instances") or []
    if not instances:
        raise RuntimeError("cursor_instances.json 无多开实例")
    return instances[0]["id"]


def profile_dir_for(instance_id: str) -> Path:
    data = json.loads(INSTANCES_JSON.read_text(encoding="utf-8"))
    for item in data.get("instances") or []:
        if item.get("id") == instance_id:
            return Path(item["userDataDir"])
    raise RuntimeError(f"实例不存在: {instance_id}")


def auth_snapshot(db_path: Path) -> dict[str, int | None]:
    if not db_path.exists():
        return {}
    conn = sqlite3.connect(db_path)
    keys = [
        "cursorAuth/accessToken",
        "cursorAuth/refreshToken",
        "cursorAuth/cachedEmail",
        "cursor.accessToken",
    ]
    out: dict[str, int | None] = {}
    for key in keys:
        row = conn.execute("select length(value) from ItemTable where key=?", (key,)).fetchone()
        out[key] = row[0] if row else None
    conn.close()
    return out


def parse_switch_log(text: str) -> dict:
    picked = re.findall(r"pick: pool=(?:full|good) candidates=\d+ picked=(cursor_[a-f0-9]+)", text)
    rotated = re.findall(
        r"\[Cursor Switch\] 自动轮换账号: instance_id=[^,]+, from_bind=[^,]+, to=[^,]+, pool=(?:full|good), remaining=(\d+)%",
        text,
    )
    persisted = "切号落盘校验通过" in text
    switch_done = "switchTokensInDb 完成" in text or "无忧传统路径换号完成(多开)" in text
    launch_done = (
        "切号后跳过二次 close，直接启动实例" in text
        or "CLI 工作区" in text
        or "launch returned" in text
        or "[Cursor Instance Start]" in text
    )
    probe_ok = "自动轮换账号" in text
    return {
        "picked_account_ids": picked,
        "rotation_events": len(rotated),
        "persist_verify": persisted,
        "switch_done": switch_done,
        "launch_done": launch_done,
        "probe_pre_ok": probe_ok,
    }


def main() -> int:
    if not DUAL_LAUNCH.is_file():
        print(f"缺少 {DUAL_LAUNCH}，请先 cargo build --release --bin dual-cursor-launch")
        return 2

    instance_id = first_instance_id()
    profile = profile_dir_for(instance_id)
    db_path = profile / "User" / "globalStorage" / "state.vscdb"
    log_path = today_log()
    offset = log_path.stat().st_size if log_path.exists() else 0

    proc = subprocess.run(
        [str(DUAL_LAUNCH), "--multi-only", instance_id],
        stdout=subprocess.DEVNULL,
        stderr=subprocess.DEVNULL,
        timeout=120,
    )

    time.sleep(3)
    if log_path.exists():
        with log_path.open("r", encoding="utf-8", errors="replace") as f:
            f.seek(offset)
            new_log = f.read()
    else:
        new_log = ""
    log_info = parse_switch_log(new_log)
    auth_after = auth_snapshot(db_path)

    has_cursor_auth = (
        (auth_after.get("cursorAuth/accessToken") or 0) > 50
        and (auth_after.get("cursorAuth/refreshToken") or 0) > 50
    )
    has_legacy_auth = (auth_after.get("cursor.accessToken") or 0) > 50

    ok = (
        proc.returncode == 0
        and bool(log_info["picked_account_ids"])
        and log_info["persist_verify"]
        and log_info["switch_done"]
        and log_info["launch_done"]
        and (has_cursor_auth or has_legacy_auth)
    )

    report = {
        "ok": ok,
        "ts": datetime.now(timezone.utc).isoformat(),
        "instance_id": instance_id,
        "profile_dir": str(profile),
        "dual_launch_exit": proc.returncode,
        "dual_launch_stderr_tail": "",
        "log": log_info,
        "auth_after_launch": auth_after,
        "has_cursor_auth": has_cursor_auth,
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
