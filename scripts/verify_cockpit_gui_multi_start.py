#!/usr/bin/env python3
"""经 Cockpit 主程序 UI 启动多开实例并验收切号落盘（非 dual-cursor-launch CLI）。"""
from __future__ import annotations

import json
import os
import sqlite3
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
if str(ROOT) not in sys.path:
    sys.path.insert(0, str(ROOT))

from scripts.cockpit_identity import verify_cockpit_identity  # noqa: E402

import uiautomation as auto

from scripts.verify_dual_cursor_instances import (  # noqa: E402
    INSTANCES_JSON,
    LOG_DIR,
    collect_cursor_cmdlines,
    dismiss_stray_modals,
    extract_user_data_dir,
    list_cockpit_windows,
    move_offscreen,
    navigate_cursor_sidebar,
    navigate_to_instances_tab,
    prepare_visible,
    stop_profile_cursor,
    ui_invoke,
    walk,
)

REPORT = ROOT / "scripts" / "cockpit_gui_multi_start_report.json"
COCKPIT_EXE = Path(
    os.environ.get(
        "COCKPIT_EXE",
        str(Path.home() / "Desktop" / "Cockpit-nirvana-token-test.exe"),
    )
)
TARGET_INSTANCE_ID = os.environ.get(
    "COCKPIT_MULTI_INSTANCE_ID", "5cc582e1-7fa7-4d7a-aef2-db9bc5a9e29c"
)


def ensure_cockpit_running(wait_s: float = 20.0) -> dict:
    if not COCKPIT_EXE.is_file():
        raise FileNotFoundError(f"验收 exe 不存在: {COCKPIT_EXE}")

    identity = verify_cockpit_identity(COCKPIT_EXE)
    if identity.get("ok"):
        return identity

    os.environ.setdefault(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--force-renderer-accessibility"
    )
    subprocess.Popen([str(COCKPIT_EXE)], env=os.environ.copy())
    deadline = time.time() + wait_s
    while time.time() < deadline:
        identity = verify_cockpit_identity(COCKPIT_EXE)
        if identity.get("ok"):
            time.sleep(2)
            return identity
        time.sleep(1)

    raise RuntimeError(
        "Cockpit 身份核对失败: "
        + "; ".join(identity.get("errors") or ["未知"])
    )


def load_instance() -> dict:
    data = json.loads(INSTANCES_JSON.read_text(encoding="utf-8"))
    for item in data.get("instances") or []:
        if item.get("id") == TARGET_INSTANCE_ID:
            return item
    raise RuntimeError(f"实例不存在: {TARGET_INSTANCE_ID}")


def click_instance_start(win: auto.Control, instance_name: str) -> tuple[bool, str]:
    """点实例卡片上的启动按钮；若已在运行则先点停止再启动。"""
    start_labels = ("启动", "Start")
    stop_labels = ("停止", "Stop")
    # 实例名在列表中可见时，优先点同区域的停止再启动
    for c in walk(win):
        if (c.Name or "").strip() != instance_name:
            continue
        try:
            row_top = c.BoundingRectangle.top
        except Exception:
            continue
        for action_labels in (stop_labels, start_labels):
            for btn in walk(win):
                n = (btn.Name or "").strip()
                if n not in action_labels:
                    continue
                try:
                    br = btn.BoundingRectangle
                    if br.width() <= 4 or br.height() <= 4:
                        continue
                    if abs(br.top - row_top) > 100:
                        continue
                except Exception:
                    continue
                if ui_invoke(btn):
                    if n in stop_labels:
                        time.sleep(2)
                        break
                    return True, n
    # 回退：点第一个可见「启动」
    for c in walk(win):
        n = (c.Name or "").strip()
        if n not in start_labels:
            continue
        try:
            r = c.BoundingRectangle
            if r.width() <= 4 or r.height() <= 4:
                continue
        except Exception:
            continue
        if ui_invoke(c):
            return True, n
    return False, ""


def auth_snapshot(db_path: Path) -> dict[str, int | None]:
    if not db_path.is_file():
        return {}
    conn = sqlite3.connect(db_path)
    keys = [
        "cursorAuth/accessToken",
        "cursorAuth/refreshToken",
        "cursorAuth/cachedEmail",
    ]
    out: dict[str, int | None] = {}
    for key in keys:
        row = conn.execute(
            "select length(value) from ItemTable where key=?", (key,)
        ).fetchone()
        out[key] = row[0] if row else None
    conn.close()
    return out


def resolve_log_path() -> Path:
    day = datetime.now().strftime("%Y-%m-%d")
    primary = LOG_DIR / f"app.log.{day}"
    if primary.is_file():
        return primary
    candidates = sorted(LOG_DIR.glob("app.log.*"), key=lambda p: p.stat().st_mtime, reverse=True)
    return candidates[0] if candidates else primary


def read_log_since_offset(offset: int) -> str:
    log_path = resolve_log_path()
    if not log_path.is_file():
        return ""
    with log_path.open("r", encoding="utf-8", errors="replace") as f:
        f.seek(offset)
        return f.read()


def main() -> int:
    inst = load_instance()
    profile = Path(inst["userDataDir"])
    db_path = profile / "User" / "globalStorage" / "state.vscdb"
    log_path = resolve_log_path()
    log_offset = log_path.stat().st_size if log_path.is_file() else 0
    since_iso = datetime.now().astimezone().isoformat(timespec="seconds")

    report: dict = {
        "ok": False,
        "cockpit_exe": str(COCKPIT_EXE),
        "instance_id": TARGET_INSTANCE_ID,
        "instance_name": inst.get("name"),
        "profile_dir": str(profile),
        "via": "cockpit_gui_ui_invoke",
    }

    stop_profile_cursor(str(profile))
    identity = ensure_cockpit_running()
    report["cockpit_identity"] = identity

    wins = list_cockpit_windows(COCKPIT_EXE)
    if not wins:
        report["error"] = "cockpit_not_running"
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        return 1

    main_win = max(wins, key=lambda w: w.BoundingRectangle.width() * w.BoundingRectangle.height())
    if not prepare_visible(main_win):
        report["error"] = "webview_focus_failed"
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        return 1

    dismiss_stray_modals(main_win)
    ok_tab, tab_label = navigate_to_instances_tab(main_win)
    if not ok_tab:
        navigate_cursor_sidebar(main_win)
        time.sleep(2)
        prepare_visible(main_win)
        ok_tab, tab_label = navigate_to_instances_tab(main_win)
    report["nav_instances"] = {"ok": ok_tab, "label": tab_label}
    if not ok_tab:
        report["error"] = "navigate_instances_failed"
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        return 1

    time.sleep(1.5)
    prepare_visible(main_win)
    ok_start, start_label = click_instance_start(main_win, str(inst.get("name") or ""))
    report["click_start"] = {"ok": ok_start, "label": start_label}
    if not ok_start:
        report["error"] = "click_start_failed"
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        return 1

    move_offscreen(main_win)
    time.sleep(25)

    new_log = read_log_since_offset(log_offset)
    report["log_since"] = since_iso
    report["log_path"] = str(log_path)
    report["log_offset"] = log_offset
    report["log_has_pick_pool_full"] = "pick: pool=full" in new_log
    report["log_has_rotation"] = "自动轮换账号" in new_log
    report["log_has_old_pick"] = "自动挑选满额账号" in new_log
    report["log_has_persist_verify"] = "切号落盘校验通过" in new_log
    report["log_has_switch_done"] = "switchTokensInDb 完成" in new_log
    report["log_has_gui_launch"] = (
        "开始启动 Cursor 实例" in new_log
        or "切号后跳过二次 close，直接启动实例" in new_log
    )

    auth_after = auth_snapshot(db_path)
    report["auth_after_launch"] = auth_after

    multi_pids = []
    norm = str(profile).replace("/", "\\").lower()
    for pid, cmd in collect_cursor_cmdlines():
        extracted = extract_user_data_dir(cmd)
        if not extracted:
            continue
        if extracted.replace("/", "\\").lower() != norm:
            continue
        if "--type=" in cmd.lower():
            continue
        multi_pids.append(pid)
    report["multi_main_pids"] = multi_pids

    has_auth = (
        (auth_after.get("cursorAuth/accessToken") or 0) > 50
        and (auth_after.get("cursorAuth/refreshToken") or 0) > 50
        and (auth_after.get("cursorAuth/cachedEmail") or 0) > 3
    )
    report["ok"] = bool(
        ok_start
        and report["log_has_pick_pool_full"]
        and report["log_has_rotation"]
        and not report["log_has_old_pick"]
        and report["log_has_switch_done"]
        and report["log_has_gui_launch"]
        and has_auth
        and len(multi_pids) > 0
    )

    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
