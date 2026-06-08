#!/usr/bin/env python3
"""Self-verify Cockpit UI: launch release exe, navigate to Cursor, capture screenshot."""
from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path

import uiautomation as auto

EXE = Path(
    r"C:\Users\aliceemoce\dev\cargo-target\cockpit-tools\release\cockpit-tools.exe"
)
OUT_DIR = Path(r"C:\Users\aliceemoce\AppData\Local\Temp")
PNG_BEFORE = OUT_DIR / "cockpit-verify-before.png"
PNG_AFTER = OUT_DIR / "cockpit-verify-cursor.png"
REPORT = OUT_DIR / "cockpit-verify-full-report.json"
NAV_DEEPLINK = "cockpit-tools://import?provider=cursor&token=ui-verify"


def kill_cockpit() -> None:
    subprocess.run(
        ["taskkill", "/F", "/IM", "cockpit-tools.exe"],
        capture_output=True,
        text=True,
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


def walk_keywords(ctrl: auto.Control) -> dict:
    names: list[str] = []

    def walk(c: auto.Control, depth: int = 0) -> None:
        if depth > 22:
            return
        try:
            n = (c.Name or "").strip()
            if n:
                names.append(n)
        except Exception:
            pass
        try:
            for ch in c.GetChildren():
                walk(ch, depth + 1)
        except Exception:
            pass

    walk(ctrl)
    keywords = ["检测", "未检测", "Cursor", "设置", "账号", "Credits", "ERR_CONNECTION"]
    return {k: [n for n in names if k in n][:12] for k in keywords}


def capture(win: auto.Control, path: Path) -> dict:
    try:
        win.SetTopmost(True)
        auto.Sleep(0.5)
    except Exception:
        pass
    win.CaptureToImage(str(path))
    hits = walk_keywords(win)
    try:
        win.SetTopmost(False)
    except Exception:
        pass
    return {
        "png": str(path),
        "png_bytes": path.stat().st_size if path.is_file() else 0,
        "keyword_hits": hits,
    }


def main() -> None:
    report: dict = {"exe": str(EXE), "exe_exists": EXE.is_file()}
    if not EXE.is_file():
        REPORT.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(json.dumps(report))
        return

    kill_cockpit()
    time.sleep(1)
    subprocess.Popen([str(EXE)], cwd=EXE.parent)
    time.sleep(10)

    win = largest_cockpit_window()
    if not win:
        report["error"] = "no_cockpit_window"
        REPORT.write_text(json.dumps(report, indent=2), encoding="utf-8")
        print(json.dumps(report))
        return

    r = win.BoundingRectangle
    report["before"] = capture(win, PNG_BEFORE)
    report["window_rect"] = {
        "width": r.width(),
        "height": r.height(),
    }

    # Navigate via second-instance deep link (single-instance plugin).
    subprocess.Popen([str(EXE), NAV_DEEPLINK], cwd=EXE.parent)
    time.sleep(6)

    win = largest_cockpit_window() or win
    report["after_cursor_nav"] = capture(win, PNG_AFTER)

    dist_glob = list(
        Path(r"C:\Users\aliceemoce\dev\cockpit-tools\dist\assets").glob(
            "PlatformOverviewTabsHeader*.js"
        )
    )
    report["bundle_badge"] = False
    if dist_glob:
        text = dist_glob[0].read_text(encoding="utf-8", errors="ignore")
        report["bundle_badge"] = "installed-version-badge" in text

    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
