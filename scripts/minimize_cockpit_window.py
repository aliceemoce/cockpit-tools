#!/usr/bin/env python3
"""Minimize Cockpit main window without mouse (UIA Invoke / WindowPattern / Win32)."""
from __future__ import annotations

import json
import sys
import time
from pathlib import Path

import uiautomation as auto

REPORT = Path(r"C:\Users\aliceemoce\AppData\Local\Temp\cockpit-minimize-report.json")


def largest_cockpit_window() -> auto.Control | None:
    best = None
    best_area = 0
    for w in auto.GetRootControl().GetChildren():
        try:
            if (w.ControlTypeName or "") != "WindowControl":
                continue
            name = (w.Name or "").lower()
            if "cockpit" not in name or "siw" in name:
                continue
            r = w.BoundingRectangle
            width = max(0, r.width())
            height = max(0, r.height())
            if width < 400 or height < 400:
                continue
            area = width * height
            if area > best_area:
                best_area = area
                best = w
        except Exception:
            pass
    return best


def find_minimize_button(win: auto.Control) -> auto.Control | None:
    queue: list[auto.Control] = [win]
    while queue:
        ctrl = queue.pop(0)
        try:
            name = (ctrl.Name or "").strip()
            ctype = ctrl.ControlTypeName or ""
            if name == "最小化" and ctype == "ButtonControl":
                return ctrl
        except Exception:
            pass
        try:
            queue.extend(ctrl.GetChildren())
        except Exception:
            pass
    return None


def minimize_via_invoke(win: auto.Control) -> str | None:
    if (win.ControlTypeName or "") != "WindowControl":
        return "not_window_control"
    btn = find_minimize_button(win)
    if not btn:
        return "minimize_button_not_found"
    pattern = btn.GetInvokePattern()
    if not pattern:
        return "invoke_pattern_unavailable"
    pattern.Invoke()
    return None


def minimize_via_window_pattern(win: auto.Control) -> str | None:
    if (win.ControlTypeName or "") != "WindowControl":
        return "not_window_control"
    pattern = win.GetWindowPattern()
    if not pattern:
        return "window_pattern_unavailable"
    pattern.SetWindowVisualState(auto.WindowVisualState.Minimized)
    return None


def minimize_via_win32(win: auto.Control) -> str | None:
    hwnd = win.NativeWindowHandle
    if not hwnd:
        return "native_hwnd_missing"
    import ctypes

    SW_MINIMIZE = 6
    ok = ctypes.windll.user32.ShowWindow(hwnd, SW_MINIMIZE)
    if ok == 0:
        return "showwindow_returned_zero"
    return None


def window_state(win: auto.Control) -> str:
    if (win.ControlTypeName or "") != "WindowControl":
        return "not_window"
    pattern = win.GetWindowPattern()
    if not pattern:
        return "unknown"
    state = pattern.WindowVisualState
    if state == auto.WindowVisualState.Minimized:
        return "minimized"
    if state == auto.WindowVisualState.Maximized:
        return "maximized"
    return "normal"


def main() -> int:
    win = largest_cockpit_window()
    if not win:
        report = {"ok": False, "error": "no_cockpit_window"}
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False))
        return 1

    before = window_state(win)
    attempts: list[dict] = []

    for method, fn in [
        ("invoke_minimize_button", minimize_via_invoke),
        ("window_pattern", minimize_via_window_pattern),
        ("win32_showwindow", minimize_via_win32),
    ]:
        err = fn(win)
        time.sleep(0.35)
        after = window_state(win)
        attempts.append({"method": method, "error": err, "state_after": after})
        if after == "minimized":
            break

    final = window_state(win)
    rect = win.BoundingRectangle
    report = {
        "ok": final == "minimized",
        "window": win.Name,
        "state_before": before,
        "state_after": final,
        "rect": {
            "x": rect.left,
            "y": rect.top,
            "width": rect.width(),
            "height": rect.height(),
        },
        "attempts": attempts,
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))
    return 0 if report["ok"] else 2


if __name__ == "__main__":
    sys.exit(main())
