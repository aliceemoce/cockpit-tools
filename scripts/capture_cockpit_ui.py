#!/usr/bin/env python3
"""Capture Cockpit main window and walk UIA tree; save report + PNG."""
from __future__ import annotations

import json
from pathlib import Path

import uiautomation as auto

OUT_DIR = Path(r"C:\Users\aliceemoce\AppData\Local\Temp")
PNG = OUT_DIR / "cockpit-verify-main.png"
REPORT = OUT_DIR / "cockpit-verify-report.json"


def largest_cockpit_window() -> auto.Control | None:
    best = None
    best_area = 0
    for w in auto.GetRootControl().GetChildren():
        try:
            if "cockpit" not in (w.Name or "").lower():
                continue
            r = w.BoundingRectangle
            area = max(0, r.width()) * max(0, r.height())
            if area > best_area:
                best_area = area
                best = w
        except Exception:
            pass
    return best


def walk(ctrl: auto.Control, depth: int = 0, acc: list | None = None, max_depth: int = 20) -> list:
    if acc is None:
        acc = []
    if depth > max_depth:
        return acc
    try:
        name = (ctrl.Name or "").strip()
        if name:
            acc.append(
                {
                    "depth": depth,
                    "type": ctrl.ControlTypeName,
                    "name": name[:200],
                }
            )
    except Exception:
        pass
    try:
        for ch in ctrl.GetChildren():
            walk(ch, depth + 1, acc, max_depth)
    except Exception:
        pass
    return acc


def main() -> None:
    win = largest_cockpit_window()
    if not win:
        REPORT.write_text(json.dumps({"error": "no cockpit window"}), encoding="utf-8")
        print("no window")
        return

    rect = win.BoundingRectangle
    try:
        win.SetTopmost(True)
        auto.Sleep(0.4)
    except Exception:
        pass

    win.CaptureToImage(str(PNG))

    names = walk(win)
    keywords = ["检测", "未检测", "Cursor", "设置", "账号", "Credits", "Cursor.exe"]
    hits = {k: [n for n in names if k in n["name"]][:8] for k in keywords}

    report = {
        "window": win.Name,
        "rect": {
            "x": rect.left,
            "y": rect.top,
            "width": rect.width(),
            "height": rect.height(),
        },
        "png": str(PNG),
        "png_bytes": PNG.stat().st_size if PNG.is_file() else 0,
        "total_named_nodes": len(names),
        "keyword_hits": hits,
        "sample_names": names[:40],
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({"png": str(PNG), "report": str(REPORT), "rect": report["rect"]}))

    try:
        win.SetTopmost(False)
    except Exception:
        pass


if __name__ == "__main__":
    main()
