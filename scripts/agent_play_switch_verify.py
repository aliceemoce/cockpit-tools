#!/usr/bin/env python3
"""最小化/离屏 Invoke 验收 Cursor 切号。优先悬浮卡「切换」，否则账号页 Play。"""
from __future__ import annotations

import json
import os
import re
import subprocess
import sys
import time
from datetime import datetime, timezone
from pathlib import Path

import uiautomation as auto
import win32con
import win32gui

INSTALLED = Path.home() / "AppData/Local/Cockpit Tools/cockpit-tools-fork-exp.exe"
REPORT = Path(__file__).resolve().parent / "agent_play_switch_report.json"
LOG_DIR = Path.home() / ".antigravity_cockpit/logs"

SWITCH_LABELS = (
    "切换到此账号",
    "Switch to this account",
    "切换到 Cursor",
    "Switch to Cursor",
    "Inject to Cursor",
)
OPEN_DETAILS_LABELS = ("打开详情页", "Open details", "打开 Cockpit Tools")
NEXT_LABELS = ("下一个账号", "Next account")


def list_cockpit_windows() -> list[auto.Control]:
    wins: list[auto.Control] = []
    for w in auto.GetRootControl().GetChildren():
        try:
            if (w.ControlTypeName or "") != "WindowControl":
                continue
            if w.ClassName != "Tauri Window":
                continue
            if "cockpit" not in (w.Name or "").lower():
                continue
            if "siw" in (w.Name or "").lower() or "sic" in (w.ClassName or "").lower():
                continue
            wins.append(w)
        except Exception:
            pass
    return wins


def find_hwnd(win: auto.Control) -> int:
    try:
        h = int(win.NativeWindowHandle or 0)
        if h:
            return h
    except Exception:
        pass
    out: list[int] = []

    def cb(hwnd, _):
        if win32gui.GetClassName(hwnd) == "Tauri Window":
            if "Cockpit" in (win32gui.GetWindowText(hwnd) or ""):
                out.append(hwnd)
        return True

    win32gui.EnumWindows(cb, None)
    return out[0] if out else 0


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


def find_target_window() -> auto.Control | None:
    """优先含悬浮卡控件的窗（主窗隐藏时常只剩 375×435 悬浮卡）。"""
    candidates = list_cockpit_windows()
    if not candidates:
        return None

    scored: list[tuple[int, auto.Control]] = []
    for w in candidates:
        score = 0
        for c in walk(w):
            try:
                cls = c.ClassName or ""
                if "floating-card-nav-button" in cls:
                    score += 100
                if "floating-card-button--primary" in cls:
                    score += 200
            except Exception:
                pass
        r = w.BoundingRectangle
        score += max(0, r.width()) * max(0, r.height()) // 10000
        scored.append((score, w))
    scored.sort(key=lambda x: x[0], reverse=True)
    return scored[0][1]


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


def init_offscreen(win: auto.Control) -> tuple[bool, tuple[int, int, int, int] | None]:
    """离屏初始化；返回 (成功, 原始窗口 rect) 以便结束后恢复。"""
    hwnd = find_hwnd(win)
    if not hwnd:
        return False, None
    saved = win32gui.GetWindowRect(hwnd)
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
    for _ in range(3):
        time.sleep(2.0)
        for c in walk(win):
            try:
                if "Chrome_RenderWidgetHostHWND" in (c.ClassName or ""):
                    c.SetFocus()
                    time.sleep(5.0)
                    nav = [
                        x
                        for x in walk(win)
                        if "floating-card-nav-button" in (x.ClassName or "")
                        or "nav-item" in (x.ClassName or "")
                    ]
                    if nav:
                        return True, saved
            except Exception:
                pass
    return False, saved


def restore_window_rect(win: auto.Control, saved: tuple[int, int, int, int] | None) -> None:
    if not saved:
        return
    hwnd = find_hwnd(win)
    if not hwnd:
        return
    left, top, right, bottom = saved
    w, h = right - left, bottom - top
    win32gui.SetWindowPos(
        hwnd,
        0,
        left,
        top,
        w,
        h,
        win32con.SWP_NOZORDER | win32con.SWP_NOACTIVATE,
    )


def click_named_button(win: auto.Control, labels: tuple[str, ...]) -> tuple[bool, str]:
    for c in walk(win):
        try:
            n = (c.Name or "").strip()
            if c.ControlTypeName != "ButtonControl":
                continue
            if n in labels or any(lb in n for lb in labels):
                if ui_invoke(c):
                    return True, n
        except Exception:
            pass
    return False, ""


def click_class_button(win: auto.Control, class_sub: str) -> tuple[bool, str]:
    for c in walk(win):
        try:
            if c.ControlTypeName != "ButtonControl":
                continue
            cls = c.ClassName or ""
            if class_sub in cls and ui_invoke(c):
                return True, cls
        except Exception:
            pass
    return False, ""


def dismiss_modals(win: auto.Control) -> None:
    for label in ("关闭", "取消", "知道了", "确定"):
        ok, _ = click_named_button(win, (label,))
        if ok:
            time.sleep(0.6)
            return


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


def click_next_floating_account(win: auto.Control) -> tuple[bool, str]:
    navs = []
    for c in walk(win):
        try:
            if c.ControlTypeName == "ButtonControl" and "floating-card-nav-button" in (
                c.ClassName or ""
            ):
                navs.append(c)
        except Exception:
            pass
    if len(navs) >= 2:
        if ui_invoke(navs[1]):
            return True, "floating-card-nav-button[1]"
    ok, label = click_named_button(win, NEXT_LABELS)
    return ok, label


def try_floating_card_switch(win: auto.Control) -> tuple[bool, str]:
    """悬浮卡：下一账号 → 切换到此账号。"""
    ok, label = click_named_button(win, SWITCH_LABELS)
    if ok:
        return True, f"direct:{label}"
    ok, label = click_next_floating_account(win)
    if ok:
        time.sleep(1.5)
        init_offscreen(win)
        ok2, label2 = click_named_button(win, SWITCH_LABELS)
        if ok2:
            return True, f"after_next:{label2}"
        ok3, cls = click_class_button(win, "floating-card-button--primary")
        if ok3:
            return True, f"after_next_primary:{cls}"
    ok, cls = click_class_button(win, "floating-card-button--primary")
    if ok:
        return True, f"primary:{cls}"
    return False, ""


def try_accounts_page_play(win: auto.Control) -> tuple[bool, str]:
    ok, label = click_named_button(win, OPEN_DETAILS_LABELS)
    if ok:
        time.sleep(4.0)
        init_offscreen(win)
        dismiss_modals(win)
    navigate_cursor_sidebar(win)
    time.sleep(2.0)
    ok, label = click_named_button(win, SWITCH_LABELS)
    if ok:
        return True, f"play:{label}"
    ok, cls = click_class_button(win, "success")
    if ok:
        return True, f"play_class:{cls}"
    return False, ""


def ensure_running() -> None:
    """仅当未运行时启动；禁止 taskkill 用户正在用的实例。"""
    r = subprocess.run(
        ["tasklist", "/FI", "IMAGENAME eq cockpit-tools-fork-exp.exe", "/FO", "CSV", "/NH"],
        capture_output=True,
        text=True,
    )
    if "cockpit-tools-fork-exp.exe" in (r.stdout or "").lower():
        return
    if not INSTALLED.is_file():
        return
    os.environ.setdefault(
        "WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS", "--force-renderer-accessibility"
    )
    subprocess.Popen([str(INSTALLED)], cwd=INSTALLED.parent)
    for _ in range(30):
        if list_cockpit_windows():
            return
        time.sleep(1)


def read_tail(n: int = 300_000) -> str:
    files = sorted(LOG_DIR.glob("app.log.*"), key=lambda p: p.stat().st_mtime, reverse=True)
    return files[0].read_text(encoding="utf-8", errors="replace")[-n:] if files else ""


def parse_events(tail: str, since: str) -> list[dict]:
    out: list[dict] = []
    started = False
    for line in tail.splitlines():
        if since in line:
            started = True
        if not started:
            continue
        if "切号后跳过二次 close" in line:
            out.append({"type": "skip_second_close", "line": line[-240:]})
        if "[Cursor Switch] 开始切换账号" in line:
            out.append({"type": "switch_start", "line": line[-240:]})
        m = re.search(
            r"\[Cursor Switch\] 切号成功: account_id=([^,]+).*elapsed=(\d+)ms",
            line,
        )
        if m:
            out.append(
                {
                    "type": "switch_success",
                    "account_id": m.group(1),
                    "elapsed_ms": int(m.group(2)),
                    "line": line[-240:],
                }
            )
    return out


def main() -> int:
    since = datetime.now(timezone.utc).astimezone().strftime("%Y-%m-%dT%H:%M")
    report: dict = {
        "ok": False,
        "mode": "minimized_offscreen_invoke",
        "log_since_marker": since,
    }

    ensure_running()
    windows = list_cockpit_windows()
    if not windows:
        report["error"] = "cockpit_window_not_found"
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 1

    clicked = False
    path = ""
    used_win: auto.Control | None = None
    report["windows_tried"] = []

    saved_rects: list[tuple[auto.Control, tuple[int, int, int, int] | None]] = []

    try:
        for win in windows:
            r = win.BoundingRectangle
            entry = {"w": r.width(), "h": r.height(), "name": win.Name}
            acc, saved = init_offscreen(win)
            saved_rects.append((win, saved))
            if not acc:
                entry["accessibility"] = False
                report["windows_tried"].append(entry)
                continue
            entry["accessibility"] = True
            dismiss_modals(win)
            clicked, path = try_floating_card_switch(win)
            entry["path"] = "floating_card"
            entry["clicked"] = clicked
            entry["detail"] = path
            report["windows_tried"].append(entry)
            if clicked:
                used_win = win
                report["path"] = "floating_card"
                break
            clicked, path = try_accounts_page_play(win)
            entry["path2"] = "accounts_page"
            entry["clicked2"] = clicked
            entry["detail2"] = path
            if clicked:
                used_win = win
                report["path"] = "accounts_page"
                break

        if used_win:
            r = used_win.BoundingRectangle
            report["window_rect"] = {"w": r.width(), "h": r.height(), "name": used_win.Name}

        report["click_switch"] = clicked
        report["click_detail"] = path

        if not clicked:
            win = find_target_window() or windows[0]
            acc, saved = init_offscreen(win)
            saved_rects.append((win, saved))
            names = sorted({(c.Name or "").strip() for c in walk(win) if (c.Name or "").strip()})
            report["ui_names_sample"] = names[:50]
            report["error"] = "switch_button_not_found"
            REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
            print(json.dumps(report, ensure_ascii=False, indent=2))
            return 1

        time.sleep(30)
        events = parse_events(read_tail(), since)
        report["log_events"] = events
        report["skip_second_close"] = any(e["type"] == "skip_second_close" for e in events)
        successes = [e for e in events if e["type"] == "switch_success"]
        if successes:
            report["switch_elapsed_ms"] = successes[-1]["elapsed_ms"]
            report["switch_account_id"] = successes[-1]["account_id"]

        report["ok"] = bool(
            clicked
            and report["skip_second_close"]
            and successes
            and successes[-1]["elapsed_ms"] < 30_000
        )

        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False, indent=2))
        return 0 if report["ok"] else 1
    finally:
        for win, saved in saved_rects:
            restore_window_rect(win, saved)


if __name__ == "__main__":
    sys.exit(main())
