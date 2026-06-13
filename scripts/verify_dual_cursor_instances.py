#!/usr/bin/env python3
"""双 Cursor 实例验收：默认 profile + 多开 profile 各一账号，Invoke 点「全部启动」。"""
from __future__ import annotations

import json
import re
import sqlite3
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import uiautomation as auto
import win32con
import win32gui

REPORT = Path(__file__).resolve().parent / "dual_instance_verify_report.json"
LOG_DIR = Path.home() / ".antigravity_cockpit" / "logs"
INSTANCES_JSON = Path.home() / ".antigravity_cockpit" / "cursor_instances.json"
ACCOUNTS_JSON = Path.home() / ".antigravity_cockpit" / "cursor_accounts.json"
DEFAULT_PROFILE = Path.home() / "AppData/Roaming/Cursor"


def walk(root: auto.Control) -> list[auto.Control]:
    out: list[auto.Control] = []
    q = [root]
    while q:
        c = q.pop(0)
        out.append(c)
        try:
            q.extend(c.GetChildren())
        except Exception:
            pass
    return out


def ui_invoke(c: auto.Control) -> bool:
    try:
        inv = c.GetInvokePattern()
        if inv:
            inv.Invoke()
            return True
    except Exception:
        pass
    try:
        leg = c.GetLegacyIAccessiblePattern()
        if leg:
            leg.DoDefaultAction()
            return True
    except Exception:
        pass
    return False


def list_cockpit_windows() -> list[auto.Control]:
    return [
        w
        for w in auto.GetRootControl().GetChildren()
        if w.ClassName == "Tauri Window" and "cockpit" in (w.Name or "").lower()
    ]


def restore_main_window(win: auto.Control) -> None:
    hwnd = int(win.NativeWindowHandle or 0)
    if not hwnd:
        return
    win32gui.ShowWindow(hwnd, win32con.SW_SHOWNORMAL)
    win32gui.SetWindowPos(
        hwnd,
        0,
        100,
        80,
        1280,
        800,
        win32con.SWP_NOZORDER,
    )


def prepare_a11y(win: auto.Control) -> bool:
    hwnd = int(win.NativeWindowHandle or 0)
    if not hwnd:
        return False
    restore_main_window(win)
    time.sleep(2)
    for c in walk(win):
        if "Chrome_RenderWidgetHostHWND" in (c.ClassName or ""):
            c.SetFocus()
            time.sleep(6)
            return True
    return False


def click_contains(win: auto.Control, needles: tuple[str, ...]) -> tuple[bool, str]:
    skip = {
        "更新此网站的自动配置文件切换首选项",
        "刷新",
        "关闭标签页",
    }
    for c in walk(win):
        n = (c.Name or "").strip()
        if not n or n in skip:
            continue
        if any(x in n for x in needles):
            if ui_invoke(c):
                return True, n
    return False, ""


def read_email_from_vscdb(profile_dir: Path) -> str | None:
    db = profile_dir / "User/globalStorage/state.vscdb"
    if not db.is_file():
        return None
    conn = sqlite3.connect(db)
    try:
        row = conn.execute(
            "select value from ItemTable where key=?",
            ("cursorAuth/cachedEmail",),
        ).fetchone()
        return row[0] if row else None
    finally:
        conn.close()


def collect_cursor_cmdlines() -> list[tuple[int, str]]:
    out: list[tuple[int, str]] = []
    ps = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"name='Cursor.exe'\" | "
            "Select-Object ProcessId,CommandLine | ConvertTo-Json -Compress",
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if ps.returncode != 0 or not ps.stdout.strip():
        return out
    raw = ps.stdout.strip()
    try:
        data = json.loads(raw)
    except json.JSONDecodeError:
        return out
    if isinstance(data, dict):
        data = [data]
    for item in data:
        pid = int(item.get("ProcessId") or 0)
        cmd = str(item.get("CommandLine") or "")
        if pid:
            out.append((pid, cmd))
    return out


def extract_user_data_dir(cmdline: str) -> str | None:
    m = re.search(r"--user-data-dir(?:=|\s+)(\"[^\"]+\"|\S+)", cmdline, re.I)
    if not m:
        return None
    return m.group(1).strip('"')


def cmdline_has_workspace(cmdline: str) -> bool:
    """启动命令行应包含工作区目录（非仅 --new-window）。"""
    home = Path.home()
    candidates = [
        str(home / "dev" / "cockpit-tools"),
        str(home / "dev"),
    ]
    lowered = cmdline.lower()
    return any(c.lower() in lowered for c in candidates)


def load_expected() -> dict:
    inst = json.loads(INSTANCES_JSON.read_text(encoding="utf-8"))
    accounts = {
        a["id"]: a.get("email")
        for a in json.loads(ACCOUNTS_JSON.read_text(encoding="utf-8")).get("accounts", [])
    }
    default_bind = inst.get("defaultSettings", {}).get("bindAccountId")
    instances = inst.get("instances", [])
    multi = instances[0] if instances else None
    return {
        "default_account_id": default_bind,
        "default_email": accounts.get(default_bind or "", ""),
        "instance_id": multi.get("id") if multi else None,
        "instance_name": multi.get("name") if multi else None,
        "instance_profile": multi.get("userDataDir") if multi else None,
        "instance_account_id": multi.get("bindAccountId") if multi else None,
        "instance_email": accounts.get((multi or {}).get("bindAccountId") or "", ""),
    }


def tail_switch_logs(since_prefix: str) -> list[str]:
    files = sorted(LOG_DIR.glob("app.log*"), key=lambda p: p.stat().st_mtime, reverse=True)
    if not files:
        return []
    lines = files[0].read_text(encoding="utf-8", errors="replace").splitlines()
    return [
        ln.strip()
        for ln in lines
        if since_prefix in ln and "[Cursor Switch]" in ln
    ][-30:]


def main() -> int:
    since = datetime.now(timezone.utc).astimezone().strftime("%Y-%m-%dT%H:%M")
    report: dict = {"ok": False, "since": since, "expected": load_expected()}

    wins = list_cockpit_windows()
    if not wins:
        report["error"] = "cockpit_not_running"
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        return 1

    main_win = max(wins, key=lambda w: w.BoundingRectangle.width() * w.BoundingRectangle.height())
    card_win = min(wins, key=lambda w: w.BoundingRectangle.width() * w.BoundingRectangle.height())

    prepare_a11y(card_win)
    click_contains(card_win, ("打开详情页", "Open details"))
    time.sleep(4)
    prepare_a11y(main_win)

    click_contains(main_win, ("关闭", "取消", "知道了", "确定"))
    ok, label = click_contains(main_win, ("Cursor",))
    report["nav_cursor"] = {"ok": ok, "label": label}
    time.sleep(2)
    ok, label = click_contains(
        main_win,
        ("多开实例", "实例", "Instances", "Multi-instance", "Multi instance"),
    )
    report["nav_instances"] = {"ok": ok, "label": label}
    time.sleep(2)
    prepare_a11y(main_win)

    ok, label = click_contains(
        main_win,
        ("全部启动", "Start all", "启动全部", "Start All"),
    )
    report["click_start_all"] = {"ok": ok, "label": label}
    time.sleep(35)

    procs = collect_cursor_cmdlines()
    report["cursor_processes"] = [
        {
            "pid": pid,
            "user_data_dir": extract_user_data_dir(cmd),
            "has_workspace": cmdline_has_workspace(cmd),
            "cmd_snippet": cmd[:240],
        }
        for pid, cmd in procs
    ]

    default_email = read_email_from_vscdb(DEFAULT_PROFILE)
    inst_profile = Path(report["expected"].get("instance_profile") or "")
    instance_email = read_email_from_vscdb(inst_profile) if inst_profile else None
    report["vscdb_emails"] = {
        "default": default_email,
        "instance": instance_email,
    }

    dirs = {p.get("user_data_dir") for p in report["cursor_processes"] if p.get("user_data_dir")}
    report["distinct_profiles"] = len(dirs)
    report["switch_log_tail"] = tail_switch_logs(since[:13])

    exp = report["expected"]
    email_ok = (
        default_email
        and instance_email
        and default_email != instance_email
        and default_email.lower() == (exp.get("default_email") or "").lower()
        and instance_email.lower() == (exp.get("instance_email") or "").lower()
    )
    dual_proc = len(procs) >= 2 and report["distinct_profiles"] >= 2
    workspace_ok = sum(1 for p in report["cursor_processes"] if p.get("has_workspace")) >= 2
    no_mass_kill = not any("按镜像名关闭" in ln or "close_cursor_nirvana" in ln for ln in report["switch_log_tail"])

    report["ok"] = bool(ok and dual_proc and email_ok and workspace_ok and no_mass_kill)
    report["checks"] = {
        "start_all_clicked": ok,
        "dual_processes": dual_proc,
        "emails_match_bind": email_ok,
        "cmdline_has_workspace": workspace_ok,
        "no_mass_close": no_mass_kill,
    }

    restore_main_window(main_win)
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    raise SystemExit(main())
