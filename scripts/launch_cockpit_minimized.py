#!/usr/bin/env python3
"""Launch Cockpit release exe then minimize without mouse."""
from __future__ import annotations

import subprocess
import time
from pathlib import Path

import uiautomation as auto

EXE = Path(
    r"C:\Users\aliceemoce\dev\cargo-target\cockpit-tools\release\cockpit-tools.exe"
)
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


def minimize_no_mouse(win: auto.Control) -> bool:
    pattern = win.GetWindowPattern()
    if pattern:
        pattern.SetWindowVisualState(auto.WindowVisualState.Minimized)
        time.sleep(0.3)
        if pattern.WindowVisualState == auto.WindowVisualState.Minimized:
            return True
    for ch in win.GetChildren():
        try:
            if (ch.Name or "").strip() == "最小化":
                inv = ch.GetInvokePattern()
                if inv:
                    inv.Invoke()
                    time.sleep(0.3)
                    p2 = win.GetWindowPattern()
                    if p2 and p2.WindowVisualState == auto.WindowVisualState.Minimized:
                        return True
        except Exception:
            pass
    hwnd = win.NativeWindowHandle
    if hwnd:
        import ctypes

        ctypes.windll.user32.ShowWindow(hwnd, 6)
        time.sleep(0.3)
        p3 = win.GetWindowPattern()
        return bool(p3 and p3.WindowVisualState == auto.WindowVisualState.Minimized)
    return False


def main() -> None:
    subprocess.run(["taskkill", "/F", "/IM", "cockpit-tools.exe"], capture_output=True)
    time.sleep(1)
    subprocess.Popen([str(EXE)], cwd=EXE.parent)
    time.sleep(8)
    win = largest_cockpit_window()
    if not win:
        print('{"ok":false,"error":"no_window"}')
        return
    ok = minimize_no_mouse(win)
    p = win.GetWindowPattern()
    state = "minimized" if p and p.WindowVisualState == auto.WindowVisualState.Minimized else "other"
    print(f'{{"ok":{str(ok).lower()},"state":"{state}"}}')


if __name__ == "__main__":
    main()
