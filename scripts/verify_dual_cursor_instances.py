#!/usr/bin/env python3
"""双 Cursor 实例验收：默认 profile + 多开 profile 各一账号，Invoke 点「全部启动」。"""
from __future__ import annotations

import json
import os
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
FORK_EXP = Path.home() / "AppData/Local/Cockpit Tools/cockpit-tools-fork-exp.exe"

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


def click_named_button(win: auto.Control, labels: tuple[str, ...]) -> tuple[bool, str]:
    for c in walk(win):
        try:
            n = (c.Name or "").strip()
            if c.ControlTypeName != "ButtonControl":
                continue
            if n in labels or any(lb in n for lb in labels):
                if control_visible(c) and ui_invoke(c):
                    return True, n
        except Exception:
            pass
    return False, ""


def collect_visible_button_names(win: auto.Control, limit: int = 40) -> list[str]:
    names: list[str] = []
    for c in walk(win):
        try:
            if c.ControlTypeName != "ButtonControl":
                continue
            n = (c.Name or "").strip()
            if n and control_visible(c):
                names.append(n)
        except Exception:
            pass
        if len(names) >= limit:
            break
    return names


def ensure_fork_exp_running(wait_s: float = 18.0) -> None:
    r = subprocess.run(
        ["tasklist", "/FI", "IMAGENAME eq cockpit-tools-fork-exp.exe", "/FO", "CSV", "/NH"],
        capture_output=True,
        text=True,
    )
    if "cockpit-tools-fork-exp.exe" not in (r.stdout or "").lower():
        if not FORK_EXP.is_file():
            return
        os.environ.setdefault(
            "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--force-renderer-accessibility"
        )
        subprocess.Popen([str(FORK_EXP)], env=os.environ.copy())
        time.sleep(wait_s)
        return
    # 已在跑：若树为空则带无障碍参数重启一次
    wins = list_cockpit_windows()
    if wins:
        prepare_visible(max(wins, key=lambda w: w.BoundingRectangle.width() * w.BoundingRectangle.height()))
        if collect_visible_button_names(wins[0], 5):
            return
    subprocess.run(
        ["powershell", "-NoProfile", "-Command", "Get-Process cockpit-tools-fork-exp -EA SilentlyContinue | Stop-Process -Force"],
        capture_output=True,
    )
    time.sleep(2)
    os.environ["WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS"] = "--force-renderer-accessibility"
    subprocess.Popen([str(FORK_EXP)], env=os.environ.copy())
    time.sleep(wait_s)


def navigate_cursor_sidebar(win: auto.Control) -> bool:
    ok, _ = click_named_button(win, ("Cursor",))
    if ok:
        time.sleep(2.5)
        return True
    ok, _ = click_named_button(win, ("更多平台", "More Platforms"))
    if ok:
        time.sleep(1.5)
        ok2, _ = click_named_button(win, ("Cursor",))
        if ok2:
            time.sleep(2.5)
            return True
    return False


def navigate_to_instances_tab(win: auto.Control) -> tuple[bool, str]:
    for attempt in range(3):
        prepare_visible(win)
        dismiss_stray_modals(win)
        ok, label = find_clickable(win, ("多开实例",), exact=True)
        if ok:
            return True, label
        if attempt == 0 or not navigate_cursor_sidebar(win):
            navigate_cursor_sidebar(win)
        time.sleep(2.0)
        prepare_visible(win)
        ok, label = find_clickable(win, ("多开实例",), exact=True)
        if ok:
            return True, label
        ok, label = click_named_button(win, ("多开实例",))
        if ok:
            return True, label
        time.sleep(2.0)
    return False, ""


def stop_profile_cursor(profile_dir: str) -> list[int]:
    norm = profile_dir.replace("/", "\\").lower()
    killed: list[int] = []
    for pid, cmd in collect_cursor_cmdlines():
        extracted = extract_user_data_dir(cmd)
        if not extracted:
            continue
        if extracted.replace("/", "\\").lower() != norm:
            continue
        subprocess.run(["taskkill", "/PID", str(pid), "/F"], capture_output=True)
        killed.append(pid)
    if killed:
        time.sleep(3)
    return killed


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


LOGIN_PAGE_MARKERS = (
    "sign in to cursor",
    "welcome to cursor",
    "welcome back",
    "the best way to code with ai",
    "登录",
    "登录以继续",
)


def is_login_page_blob(blob: str) -> bool:
    lowered = blob.lower()
    if any(m in lowered for m in LOGIN_PAGE_MARKERS):
        return True
    return "log in" in lowered and "sign up" in lowered


def main_cursor_pids_for_profile(profile_dir: str) -> list[int]:
    norm = profile_dir.replace("/", "\\").lower()
    pids: list[int] = []
    for pid, cmd in collect_cursor_cmdlines():
        extracted = extract_user_data_dir(cmd)
        if not extracted:
            continue
        if extracted.replace("/", "\\").lower() != norm:
            continue
        if "--type=" in cmd.lower():
            continue
        pids.append(pid)
    return pids


def collect_names_for_pid(pid: int) -> list[str]:
    names: list[str] = []
    for w in auto.GetRootControl().GetChildren():
        try:
            if int(w.ProcessId or 0) != pid:
                continue
            for c in walk(w):
                n = (c.Name or "").strip()
                if n:
                    names.append(n)
        except Exception:
            continue
    return names


def check_not_login_page(profile_dir: str) -> tuple[bool, dict]:
    """多开主进程窗口树中不应出现登录页关键字，且应出现工作区相关文本。"""
    pids = main_cursor_pids_for_profile(profile_dir)
    all_names: list[str] = []
    for pid in pids:
        all_names.extend(collect_names_for_pid(pid))
    blob = "\n".join(all_names).lower()
    login_hits = [m for m in LOGIN_PAGE_MARKERS if m in blob]
    if is_login_page_blob(blob):
        if not login_hits:
            login_hits = ["log_in+sign_up"]
    workspace_hit = "cockpit-tools" in blob or str(Path.home() / "dev").lower() in blob
    ok = not login_hits and (workspace_hit or len(pids) > 0)
    return ok, {
        "pids": pids,
        "login_hits": login_hits,
        "workspace_hit": workspace_hit,
        "sample_names": all_names[:20],
    }


def load_fixed_bind_accounts() -> dict:
    inst = json.loads(INSTANCES_JSON.read_text(encoding="utf-8"))
    multi = inst.get("instances", [{}])[0] if inst.get("instances") else {}
    return {
        "default_account_id": inst.get("defaultSettings", {}).get("bindAccountId"),
        "instance_account_id": multi.get("bindAccountId"),
        "written_at": datetime.now(timezone.utc).astimezone().isoformat(),
    }


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
    report: dict = {
        "ok": False,
        "since": since,
        "expected": load_expected(),
        "fixed_bind_accounts": load_fixed_bind_accounts(),
    }
    ensure_fork_exp_running()
    inst_profile = report["expected"].get("instance_profile") or ""
    if inst_profile:
        report["stopped_multi_pids"] = stop_profile_cursor(str(inst_profile))

    report["default_pids_before"] = snapshot_default_pids()
    report["default_email_before"] = read_email_from_vscdb(DEFAULT_PROFILE)

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

    ok, label = navigate_to_instances_tab(main_win)
    report["nav_instances"] = {"ok": ok, "label": label}
    if not ok:
        report["visible_buttons_sample"] = collect_visible_button_names(main_win)
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
    report["default_email_after"] = default_email
    report["vscdb_emails"] = {
        "default": default_email,
        "instance": instance_email,
    }

    not_login_ok, not_login_detail = check_not_login_page(str(inst_profile))
    report["not_login_page"] = {"ok": not_login_ok, **not_login_detail}

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
    default_email_unchanged = (
        bool(report.get("default_email_before"))
        and report.get("default_email_before") == default_email
    )

    start_flow_ok = report["click_start_all"]["ok"] and report["confirm_start_all"]["ok"]
    failed = [
        k
        for k, v in {
            "nav_instances": report["nav_instances"]["ok"],
            "start_all_clicked": report["click_start_all"]["ok"],
            "confirm_dialog_clicked": report["confirm_start_all"]["ok"],
            "dual_processes": dual_proc,
            "emails_match_bind": email_ok,
            "multi_has_workspace": multi_workspace_ok,
            "default_profile_online": default_profile_ok,
            "default_pids_preserved": default_preserved,
            "default_email_unchanged": default_email_unchanged,
            "not_login_page": not_login_ok,
            "no_mass_close": no_mass_kill,
        }.items()
        if not v
    ]
    report["failed_checks"] = failed

    report["ok"] = bool(start_flow_ok and not failed)
    report["checks"] = {
        "nav_instances": report["nav_instances"]["ok"],
        "start_all_clicked": report["click_start_all"]["ok"],
        "confirm_dialog_clicked": report["confirm_start_all"]["ok"],
        "dual_processes": dual_proc,
        "emails_match_bind": email_ok,
        "multi_has_workspace": multi_workspace_ok,
        "default_profile_online": default_profile_ok,
        "default_pids_preserved": default_preserved,
        "default_email_unchanged": default_email_unchanged,
        "not_login_page": not_login_ok,
        "no_mass_close": no_mass_kill,
    }

    src_candidates = [
        Path(__file__).resolve().parents[1] / "src-tauri" / "target" / "release" / "cockpit-tools.exe",
        Path(__file__).resolve().parents[1] / "target" / "release" / "cockpit-tools.exe",
    ]
    src_exe = next((p for p in src_candidates if p.is_file()), None)
    dst_exe = Path.home() / "AppData/Local/Cockpit Tools/cockpit-tools-fork-exp.exe"
    if src_exe and dst_exe.is_file():
        report["deploy_proof"] = {
            "src_path": str(src_exe),
            "src_size": src_exe.stat().st_size,
            "dst_size": dst_exe.stat().st_size,
            "sizes_match": src_exe.stat().st_size == dst_exe.stat().st_size,
        }
    report["github_url"] = (
        "https://github.com/aliceemoce/cockpit-tools/commits/fork-on-upstream-0256"
    )
    report["cockpit_running"] = bool(
        subprocess.run(
            ["powershell", "-NoProfile", "-Command", "(Get-Process cockpit-tools-fork-exp -EA SilentlyContinue).Count"],
            capture_output=True,
            text=True,
        ).stdout.strip()
        not in ("", "0")
    )

    show_main_window(main_win)
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.stdout.reconfigure(encoding="utf-8")
    raise SystemExit(main())
