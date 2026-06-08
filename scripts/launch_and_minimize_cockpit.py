#!/usr/bin/env python3
"""Launch Cockpit, wait for main window, minimize without mouse, write report."""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

import uiautomation as auto

EXE = Path(
    r"C:\Users\aliceemoce\dev\cargo-target\cockpit-tools\release\cockpit-tools.exe"
)
REPORT = Path(__file__).resolve().parent / "minimize_run_result.json"


def find_main_window(timeout_s: float = 20.0) -> auto.Control | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
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
        if best:
            return best
        time.sleep(0.5)
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


def find_minimize_button(win: auto.Control) -> auto.Control | None:
    queue: list[auto.Control] = [win]
    while queue:
        ctrl = queue.pop(0)
        try:
            if (ctrl.Name or "").strip() == "最小化" and ctrl.ControlTypeName == "ButtonControl":
                return ctrl
        except Exception:
            pass
        try:
            queue.extend(ctrl.GetChildren())
        except Exception:
            pass
    return None


def minimize_no_mouse(win: auto.Control) -> tuple[str, str | None]:
    if (win.ControlTypeName or "") != "WindowControl":
        return "not_window_control", None

    btn = find_minimize_button(win)
    if btn:
        inv = btn.GetInvokePattern()
        if inv:
            inv.Invoke()
            time.sleep(0.4)
            if window_state(win) == "minimized":
                return "invoke_minimize_button", None

    pattern = win.GetWindowPattern()
    if pattern:
        pattern.SetWindowVisualState(auto.WindowVisualState.Minimized)
        time.sleep(0.4)
        if window_state(win) == "minimized":
            return "window_pattern", None

    hwnd = win.NativeWindowHandle
    if hwnd:
        import ctypes

        ctypes.windll.user32.ShowWindow(hwnd, 6)
        time.sleep(0.4)
        if window_state(win) == "minimized":
            return "win32_showwindow", None

    return "all_failed", "could_not_minimize"


def main() -> int:
    subprocess.run(["taskkill", "/F", "/IM", "cockpit-tools.exe"], capture_output=True)
    time.sleep(1)
    subprocess.Popen([str(EXE)], cwd=EXE.parent)
    win = find_main_window(25.0)
    if not win:
        report = {"ok": False, "error": "no_main_window_after_launch"}
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False))
        return 1

    before = window_state(win)
    method, err = minimize_no_mouse(win)
    after = window_state(win)
    r = win.BoundingRectangle
    report = {
        "ok": after == "minimized",
        "window": win.Name,
        "state_before": before,
        "state_after": after,
        "success_method": method if after == "minimized" else None,
        "error": err,
        "rect_after": {
            "x": r.left,
            "y": r.top,
            "width": r.width(),
            "height": r.height(),
        },
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))
    return 0 if report["ok"] else 2


if __name__ == "__main__":
    sys.exit(main())
