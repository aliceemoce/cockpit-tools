#!/usr/bin/env python3
"""通过 Cockpit 切号 → 在 Cursor 里发消息 → 读回复，证明已登录可用。"""
from __future__ import annotations

import json
import os
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import uiautomation as auto
import win32con
import win32gui
import win32process

INSTALLED = Path.home() / "AppData/Local/Cockpit Tools/cockpit-tools.exe"
RELEASE = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\target\release\cockpit-tools.exe")
REPORT = Path(__file__).resolve().parent / "cursor_logged_in_chat_report.json"
LOG_DIR = Path.home() / ".antigravity_cockpit/logs"
PROMPT = "Reply with exactly one word: PONG"
SWITCH_LABELS = (
    "切换到此账号",
    "Switch to this account",
    "切换到 Cursor",
    "Switch to Cursor",
)


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


def label(c: auto.Control) -> str:
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


def find_cockpit() -> auto.Control | None:
    for w in auto.GetRootControl().GetChildren():
        try:
            if w.ClassName == "Tauri Window" and "cockpit" in (w.Name or "").lower():
                return w
        except Exception:
            pass
    return None


def click_labels(root: auto.Control, labels: tuple[str, ...]) -> bool:
    for c in walk(root):
        text = label(c)
        if text and any(lb in text for lb in labels):
            if ui_invoke(c):
                return True
    return False


def init_webview_a11y(win: auto.Control) -> None:
    for c in walk(win):
        try:
            if "Chrome_RenderWidgetHostHWND" in (c.ClassName or ""):
                c.SetFocus()
                time.sleep(3)
                return
        except Exception:
            pass


def cursor_windows() -> list[tuple[int, str]]:
    out: list[tuple[int, str]] = []

    def cb(hwnd, _):
        try:
            if not win32gui.IsWindowVisible(hwnd):
                return True
            title = win32gui.GetWindowText(hwnd) or ""
            cls = win32gui.GetClassName(hwnd)
            if cls != "Chrome_WidgetWin_1":
                return True
            tl = title.lower()
            if "cockpit" in tl:
                return True
            _, pid = win32process.GetWindowThreadProcessId(hwnd)
            try:
                import psutil

                name = psutil.Process(pid).name().lower()
            except Exception:
                name = ""
            if name == "cursor.exe" or "cursor" in tl:
                out.append((hwnd, title))
        except Exception:
            pass
        return True

    win32gui.EnumWindows(cb, None)
    return out


def is_login_title(title: str) -> bool:
    tl = title.lower()
    return any(x in tl for x in ("sign in", "log in", "登录", "login"))


def focus_hwnd(hwnd: int) -> None:
    win32gui.ShowWindow(hwnd, win32con.SW_RESTORE)
    try:
        win32gui.SetForegroundWindow(hwnd)
    except Exception:
        pass
    time.sleep(0.5)


def send_chat_via_keyboard(hwnd: int) -> tuple[bool, str]:
    """Ctrl+L 打开 Chat，输入问题，Enter 发送，轮询界面文本。"""
    try:
        import pyautogui

        pyautogui.FAILSAFE = False
    except ImportError:
        return False, "缺少 pyautogui"

    focus_hwnd(hwnd)
    time.sleep(1)
    pyautogui.hotkey("ctrl", "l")
    time.sleep(1.5)
    pyautogui.write(PROMPT, interval=0.02)
    time.sleep(0.3)
    pyautogui.press("enter")
    time.sleep(12)

    texts: list[str] = []
    for c in walk(auto.ControlFromHandle(hwnd)):
        try:
            t = label(c)
            if t and len(t) > 2:
                texts.append(t)
        except Exception:
            pass
    blob = "\n".join(texts).lower()
    if "pong" in blob:
        return True, "回复含 PONG"
    if is_login_title(win32gui.GetWindowText(hwnd)):
        return False, "仍在登录页"
    if "sign in" in blob or "log in" in blob or "登录" in blob:
        return False, "界面仍含登录文案"
    return False, f"未见 PONG，采样文本行数={len(texts)}"


def latest_log_tail(n: int = 80) -> str:
    if not LOG_DIR.is_dir():
        return ""
    files = sorted(LOG_DIR.glob("app.log.*"), key=lambda p: p.stat().st_mtime, reverse=True)
    if not files:
        return ""
    lines = files[0].read_text(encoding="utf-8", errors="replace").splitlines()
    return "\n".join(lines[-n:])


def main() -> int:
    errors: list[str] = []
    notes: list[str] = []

    exe = INSTALLED if INSTALLED.is_file() else RELEASE
    if not exe.is_file():
        errors.append(f"找不到 Cockpit exe: {exe}")
    else:
        notes.append(f"exe={exe}")

    os.environ.setdefault("WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--force-renderer-accessibility")
    tl = subprocess.run(
        ["tasklist", "/FI", "IMAGENAME eq cockpit-tools.exe", "/NH"],
        capture_output=True,
        text=True,
        check=False,
    )
    if "cockpit-tools.exe" not in (tl.stdout or "").lower():
        subprocess.Popen([str(exe)], cwd=exe.parent)
        notes.append("已启动 Cockpit")
        time.sleep(8)

    win = find_cockpit()
    if not win:
        errors.append("未找到 Cockpit 窗口")
    else:
        notes.append(f"Cockpit={win.Name}")
        init_webview_a11y(win)
        click_labels(win, ("Cursor",))
        time.sleep(2)
        if not click_labels(win, SWITCH_LABELS):
            notes.append("未点到 Play/切换按钮，尝试继续检测已有 Cursor")
        else:
            notes.append("已 Invoke 切换按钮")
            time.sleep(18)

    before = {h for h, _ in cursor_windows()}
    time.sleep(3)
    after = cursor_windows()
    notes.append(f"Cursor 窗口数={len(after)}")
    for hwnd, title in after:
        notes.append(f"  hwnd={hwnd} title={title[:80]}")

    target = None
    for hwnd, title in after:
        if is_login_title(title):
            errors.append(f"Cursor 窗口为登录页: {title}")
            continue
        target = (hwnd, title)
        break

    if target is None and not errors:
        errors.append("未发现非登录态 Cursor 窗口")

    chat_ok = False
    chat_note = ""
    if target:
        chat_ok, chat_note = send_chat_via_keyboard(target[0])
        notes.append(f"对话验证: {chat_note}")
        if not chat_ok:
            errors.append(f"Cursor 对话未证明已登录: {chat_note}")

    tail = latest_log_tail()
    if "无忧传统路径换号完成" in tail or "switchTokensInDb 完成" in tail:
        notes.append("日志含切号链")
    elif "切号成功" in tail:
        notes.append("日志含切号成功")

    report = {
        "ok": len(errors) == 0,
        "errors": errors,
        "notes": notes,
        "verified_at": datetime.now(timezone.utc).isoformat(),
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
