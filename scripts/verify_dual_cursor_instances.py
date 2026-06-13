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

# 全部启动后的 Tauri 原生确认框（不在 WebView 树内）
BULK_START_CONFIRM_TEXTS = (
    "确认启动所有未运行实例",
    "Start all stopped instances",
    "是否啟動所有已停止的實例",
)
CONFIRM_OK_LABELS = ("确定", "確認", "OK", "Confirm", "Yes", "是")


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


def show_main_window(win: auto.Control) -> None:
    """先全屏/正常尺寸摆到可见区域，再操作控件（用户要求：不能先离屏）。"""
    hwnd = int(win.NativeWindowHandle or 0)
    if not hwnd:
        return
    win32gui.ShowWindow(hwnd, win32con.SW_RESTORE)
    win32gui.ShowWindow(hwnd, win32con.SW_SHOWNORMAL)
    win32gui.SetWindowPos(
        hwnd,
        win32con.HWND_TOP,
        80,
        60,
        1280,
        800,
        win32con.SWP_SHOWWINDOW,
    )
    time.sleep(1.5)


def move_offscreen(win: auto.Control) -> None:
    hwnd = int(win.NativeWindowHandle or 0)
    if not hwnd:
        return
    win32gui.SetWindowPos(
        hwnd,
        0,
        -32000,
        -32000,
        0,
        0,
        win32con.SWP_NOSIZE | win32con.SWP_NOZORDER | win32con.SWP_NOACTIVATE,
    )
    win32gui.ShowWindow(hwnd, win32con.SW_SHOWNOACTIVATE)


def focus_webview(win: auto.Control, wait_s: float = 8.0) -> bool:
    for c in walk(win):
        if "Chrome_RenderWidgetHostHWND" in (c.ClassName or ""):
            c.SetFocus()
            time.sleep(wait_s)
            return True
    return False


def prepare_visible(win: auto.Control) -> bool:
    show_main_window(win)
    return focus_webview(win)


def control_visible(c: auto.Control) -> bool:
    try:
        r = c.BoundingRectangle
        return r.width() > 4 and r.height() > 4
    except Exception:
        return False


def find_clickable(
    win: auto.Control,
    needles: tuple[str, ...],
    *,
    exact: bool = False,
    exclude: tuple[str, ...] = (),
) -> tuple[bool, str]:
    skip = {
        "更新此网站的自动配置文件切换首选项",
        "刷新",
        "关闭标签页",
        "Cursor 账号管理说明",
        "一键启动",
    }
    skip.update(exclude)
    for c in walk(win):
        n = (c.Name or "").strip()
        if not n or n in skip or not control_visible(c):
            continue
        if any(x in n for x in exclude):
            continue
        matched = n in needles if exact else any(x in n for x in needles)
        if matched and ui_invoke(c):
            return True, n
    return False, ""


def find_native_confirm_dialog() -> auto.Control | None:
    """Tauri plugin-dialog 原生确认框，独立于主 WebView。"""
    try:
        children = auto.GetRootControl().GetChildren()
    except Exception:
        children = []
    for w in children:
        try:
            name = (w.Name or "").strip()
            cls = w.ClassName or ""
        except Exception:
            continue
        if cls == "#32770" or "dialog" in cls.lower():
            try:
                body = name + " " + " ".join((c.Name or "") for c in walk(w)[:40])
            except Exception:
                body = name
            if any(t in body for t in BULK_START_CONFIRM_TEXTS):
                return w
        if any(t in name for t in BULK_START_CONFIRM_TEXTS):
            return w
    return None


def find_native_confirm_hwnd() -> int | None:
    hits: list[int] = []

    def _enum(hwnd: int, _: int) -> bool:
        try:
            if not win32gui.IsWindowVisible(hwnd):
                return True
            title = win32gui.GetWindowText(hwnd) or ""
            cls = win32gui.GetClassName(hwnd) or ""
            if cls == "#32770" or any(t in title for t in BULK_START_CONFIRM_TEXTS):
                hits.append(hwnd)
            elif "Cockpit Tools" in title and cls == "#32770":
                hits.append(hwnd)
        except Exception:
            pass
        return True

    win32gui.EnumWindows(_enum, 0)
    return hits[0] if hits else None


def click_native_confirm(timeout_s: float = 12.0) -> tuple[bool, str]:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        hwnd = find_native_confirm_hwnd()
        if hwnd:
            for label in CONFIRM_OK_LABELS:
                btn = win32gui.FindWindowEx(hwnd, 0, "Button", label)
                if btn:
                    win32gui.SendMessage(btn, win32con.BM_CLICK, 0, 0)
                    return True, label
            win32gui.PostMessage(hwnd, win32con.WM_COMMAND, 1, 0)
            return True, "WM_COMMAND:1"
        try:
            dlg = find_native_confirm_dialog()
            if dlg:
                for c in walk(dlg):
                    n = (c.Name or "").strip()
                    if n in CONFIRM_OK_LABELS and ui_invoke(c):
                        return True, n
        except Exception:
            pass
        time.sleep(0.4)
    return False, ""


def dismiss_stray_modals(win: auto.Control) -> None:
    for _ in range(3):
        ok, _ = find_clickable(win, ("关闭", "取消", "知道了"), exact=True)
        if not ok:
            break
        time.sleep(0.8)


def snapshot_default_pids() -> list[int]:
    ps = subprocess.run(
        [
            "powershell",
            "-NoProfile",
            "-Command",
            "Get-CimInstance Win32_Process -Filter \"name='Cursor.exe'\" | "
            "Where-Object { $_.CommandLine -notmatch 'user-data-dir' -or $_.CommandLine -match 'Roaming\\\\Cursor' } | "
            "Select-Object -ExpandProperty ProcessId | ConvertTo-Json -Compress",
        ],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    if ps.returncode != 0 or not ps.stdout.strip():
        return []
    try:
        data = json.loads(ps.stdout.strip())
    except json.JSONDecodeError:
        return []
    if isinstance(data, int):
        return [data]
    if isinstance(data, list):
        return [int(x) for x in data if str(x).isdigit()]
    return []


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
    report["default_pids_before"] = snapshot_default_pids()

    wins = list_cockpit_windows()
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

    ok, label = find_clickable(main_win, ("多开实例",), exact=True)
    report["nav_instances"] = {"ok": ok, "label": label}
    if not ok:
        ok_side, label_side = find_clickable(main_win, ("Cursor",), exact=True)
        report["nav_cursor_sidebar"] = {"ok": ok_side, "label": label_side}
        time.sleep(2)
        prepare_visible(main_win)
        ok, label = find_clickable(main_win, ("多开实例",), exact=True)
        report["nav_instances"] = {"ok": ok, "label": label}
    time.sleep(2)
    prepare_visible(main_win)

    ok, label = find_clickable(
        main_win,
        ("全部启动", "Start All", "Start all"),
        exact=True,
    )
    report["click_start_all"] = {"ok": ok, "label": label}

    confirm_ok, confirm_label = (False, "")
    if ok:
        confirm_ok, confirm_label = click_native_confirm(timeout_s=15.0)
    report["confirm_start_all"] = {"ok": confirm_ok, "label": confirm_label}

    move_offscreen(main_win)
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
    multi_workspace_ok = any(
        p.get("has_workspace")
        and p.get("user_data_dir")
        and "instances" in str(p.get("user_data_dir")).lower()
        for p in report["cursor_processes"]
    )
    default_profile_ok = any(
        p.get("user_data_dir")
        and "roaming\\cursor" in str(p.get("user_data_dir")).lower().replace("/", "\\")
        for p in report["cursor_processes"]
    )
    workspace_ok = multi_workspace_ok and default_profile_ok
    no_mass_kill = not any(
        "按镜像名关闭" in ln or "close_cursor_nirvana" in ln for ln in report["switch_log_tail"]
    )

    report["default_pids_after"] = snapshot_default_pids()
    preserved = [pid for pid in report["default_pids_before"] if pid in report["default_pids_after"]]
    report["default_pids_preserved_count"] = len(preserved)
    default_preserved = len(preserved) == len(report["default_pids_before"]) and len(preserved) > 0

    start_flow_ok = report["click_start_all"]["ok"] and report["confirm_start_all"]["ok"]
    report["ok"] = bool(
        start_flow_ok
        and report["nav_instances"]["ok"]
        and dual_proc
        and email_ok
        and workspace_ok
        and no_mass_kill
        and default_preserved
    )
    report["checks"] = {
        "nav_instances": report["nav_instances"]["ok"],
        "start_all_clicked": report["click_start_all"]["ok"],
        "confirm_dialog_clicked": report["confirm_start_all"]["ok"],
        "dual_processes": dual_proc,
        "emails_match_bind": email_ok,
        "multi_has_workspace": multi_workspace_ok,
        "default_profile_online": default_profile_ok,
        "default_pids_preserved": default_preserved,
        "no_mass_close": no_mass_kill,
    }

    show_main_window(main_win)
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    raise SystemExit(main())
