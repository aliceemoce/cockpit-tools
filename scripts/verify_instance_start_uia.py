#!/usr/bin/env python3
"""无鼠标 UI Automation：Cursor 多开 Start（不用总览 Play），验无忧切号链 + Cursor 是否启动。"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import uiautomation as auto
import win32con
import win32gui

RELEASE = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\target\release\cockpit-tools.exe")
INSTALLED = Path.home() / "AppData/Local/Cockpit Tools/cockpit-tools.exe"
REPORT = Path(__file__).resolve().parent / "instance_start_verify_report.json"
LOG_DIR = Path.home() / ".antigravity_cockpit/logs"

CURSOR_SIDEBAR = ("Cursor",)
INSTANCES_TAB = ("多开实例", "Instances", "多开")
START_HINTS = ("启动", "Start", "Play")
LOGIN_FAIL_HINTS = ("sign in", "log in", "登录", "login")


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


def ctrl_label(c: auto.Control) -> str:
    parts: list[str] = []
    try:
        if c.Name:
            parts.append(c.Name.strip())
    except Exception:
        pass
    try:
        leg = c.GetLegacyIAccessiblePattern()
        if leg and leg.CurrentName:
            parts.append(str(leg.CurrentName).strip())
    except Exception:
        pass
    return " | ".join(p for p in parts if p)


def ui_invoke(ctrl: auto.Control) -> bool:
    try:
        inv = ctrl.GetInvokePattern()
        if inv:
            inv.Invoke()
            return True
    except Exception:
        pass
    try:
        leg = ctrl.GetLegacyIAccessiblePattern()
        if leg:
            leg.DoDefaultAction()
            return True
    except Exception:
        pass
    return False


def find_cockpit_window(timeout: float = 50.0) -> auto.Control | None:
    deadline = time.time() + timeout
    while time.time() < deadline:
        for w in auto.GetRootControl().GetChildren():
            try:
                if w.ClassName != "Tauri Window":
                    continue
                if "cockpit" in (w.Name or "").lower():
                    return w
            except Exception:
                pass
        time.sleep(0.5)
    return None


def click_by_labels(root: auto.Control, labels: tuple[str, ...]) -> tuple[bool, str]:
    for ctrl in walk(root):
        label = ctrl_label(ctrl)
        if not label:
            continue
        if any(lb in label for lb in labels):
            if ui_invoke(ctrl):
                return True, label
    return False, ""


def latest_log() -> Path | None:
    if not LOG_DIR.is_dir():
        return None
    files = sorted(LOG_DIR.glob("*.log"), key=lambda p: p.stat().st_mtime, reverse=True)
    return files[0] if files else None


def log_tail(path: Path, n: int = 250) -> str:
    try:
        lines = path.read_text(encoding="utf-8", errors="replace").splitlines()
        return "\n".join(lines[-n:])
    except OSError:
        return ""


def cursor_running() -> bool:
    r = subprocess.run(
        ["tasklist", "/FI", "IMAGENAME eq Cursor.exe", "/FO", "CSV", "/NH"],
        capture_output=True,
        text=True,
    )
    return "cursor.exe" in (r.stdout or "").lower()


def cursor_login_window_detected() -> bool:
    hits: list[str] = []

    def cb(hwnd, _):
        try:
            if not win32gui.IsWindowVisible(hwnd):
                return True
            title = (win32gui.GetWindowText(hwnd) or "").lower()
            cls = win32gui.GetClassName(hwnd)
            if "chrome" not in cls.lower() and cls != "Chrome_WidgetWin_1":
                return True
            if "cursor" in title or cls == "Chrome_WidgetWin_1":
                if any(h in title for h in LOGIN_FAIL_HINTS):
                    hits.append(title)
        except Exception:
            pass
        return True

    win32gui.EnumWindows(cb, None)
    return len(hits) > 0


def deploy_release() -> tuple[bool, str]:
    if not RELEASE.is_file():
        return False, f"release 不存在: {RELEASE}"
    subprocess.run(["taskkill", "/F", "/IM", "cockpit-tools.exe"], capture_output=True)
    time.sleep(2)
    INSTALLED.parent.mkdir(parents=True, exist_ok=True)
    try:
        import shutil

        shutil.copy2(RELEASE, INSTALLED)
        return True, str(INSTALLED)
    except OSError as err:
        return False, str(err)


def main() -> int:
    errors: list[str] = []
    notes: list[str] = []

    ok, msg = deploy_release()
    notes.append(f"deploy: {msg}")
    if not ok:
        errors.append(f"部署失败: {msg}")

    subprocess.Popen([str(INSTALLED if INSTALLED.is_file() else RELEASE)], stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
    notes.append("已启动 Cockpit")
    time.sleep(8)

    win = find_cockpit_window()
    if not win:
        errors.append("未找到 Cockpit 窗口")
    else:
        notes.append(f"窗口: {win.Name}")
        click_by_labels(win, CURSOR_SIDEBAR)
        time.sleep(2)
        ok_tab, tab_label = click_by_labels(win, INSTANCES_TAB)
        notes.append(f"多开 Tab: {tab_label or '未点到（可能已在页内）'}")
        time.sleep(2)
        ok_start, start_label = click_by_labels(win, START_HINTS)
        if ok_start:
            notes.append(f"已点击启动: {start_label}")
            time.sleep(15)
        else:
            errors.append("未找到多开实例「启动」按钮（icon-button title）")

    if cursor_running():
        notes.append("Cursor.exe 进程已启动")
        if cursor_login_window_detected():
            errors.append("Cursor 窗口标题含登录/sign in—— 实例未能直接使用")
        else:
            notes.append("未发现明显登录页标题（已登录或主界面）")
    else:
        errors.append("点击后未发现 Cursor.exe 进程")

    log_path = latest_log()
    if log_path:
        text = log_tail(log_path)
        notes.append(f"log={log_path.name}")
        if "[Cursor Switch] 无忧传统路径换号完成" in text or "switchTokensInDb 完成" in text:
            notes.append("日志含无忧切号链标记")
        elif "closeCursor 完成" in text or "close_cursor_nirvana_style" in text:
            notes.append("日志含 closeCursor")
        else:
            errors.append("日志未见无忧切号完成标记")
    else:
        notes.append(f"无日志目录 {LOG_DIR}（首次运行或未写日志）")

    report = {
        "ok": len(errors) == 0,
        "errors": errors,
        "notes": notes,
        "verified_at": datetime.now(timezone.utc).isoformat(),
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")

    if errors:
        print("FAIL:")
        for e in errors:
            print(f"  - {e}")
        for n in notes:
            print(f"  note: {n}")
        return 1
    print("PASS:")
    for n in notes:
        print(f"  {n}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
