#!/usr/bin/env python3
"""确认 Cockpit 已真实启动（进程 + 主窗口），可选最小化。无页面导航。"""
from __future__ import annotations

import json
import subprocess
import sys
import time
from pathlib import Path

import uiautomation as auto

REPORT = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\scripts\app_launch_verify.json")
RELEASE = Path(r"C:\Users\aliceemoce\dev\cargo-target\cockpit-tools\release\cockpit-tools.exe")
INSTALLED = Path.home() / "AppData/Local/Cockpit Tools/cockpit-tools.exe"


def find_cockpit_processes() -> list[dict]:
    r = subprocess.run(
        ["tasklist", "/FI", "IMAGENAME eq cockpit-tools.exe", "/FO", "CSV", "/NH"],
        capture_output=True,
        text=True,
        encoding="utf-8",
        errors="replace",
    )
    out = []
    for line in (r.stdout or "").splitlines():
        parts = [p.strip('"') for p in line.split('","')]
        if len(parts) >= 2 and parts[0].lower() == "cockpit-tools.exe":
            out.append({"name": parts[0], "pid": parts[1].strip('"')})
    return out


def find_main_window() -> auto.Control | None:
    best = None
    best_area = 0
    for w in auto.GetRootControl().GetChildren():
        try:
            if (w.ControlTypeName or "") != "WindowControl":
                continue
            name = (w.Name or "")
            if "Cockpit" not in name:
                continue
            r = w.BoundingRectangle
            area = max(0, r.width()) * max(0, r.height())
            if area > best_area:
                best_area = area
                best = w
        except Exception:
            pass
    return best


def minimize(win: auto.Control) -> bool:
    for c in [win] + list(win.GetChildren()):
        try:
            if (c.Name or "").strip() == "最小化" and c.ControlTypeName == "ButtonControl":
                inv = c.GetInvokePattern()
                if inv:
                    inv.Invoke()
                    time.sleep(0.4)
                    return True
        except Exception:
            pass
    p = win.GetWindowPattern()
    if p:
        p.SetWindowVisualState(auto.WindowVisualState.Minimized)
        return True
    return False


def main() -> int:
    procs = find_cockpit_processes()
    win = find_main_window()
    report = {
        "process_count": len(procs),
        "processes": procs,
        "release_exe": str(RELEASE),
        "installed_exe": str(INSTALLED),
        "window_found": win is not None,
        "window_title": (win.Name if win else None),
        "window_state": None,
        "minimized": False,
        "ok": len(procs) > 0 and win is not None,
    }
    if win:
        p = win.GetWindowPattern()
        if p:
            s = p.WindowVisualState
            if s == auto.WindowVisualState.Minimized:
                report["window_state"] = "minimized"
            elif s == auto.WindowVisualState.Maximized:
                report["window_state"] = "maximized"
            else:
                report["window_state"] = "normal"
        report["minimized"] = minimize(win)
        if report["minimized"]:
            report["window_state"] = "minimized"

    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
