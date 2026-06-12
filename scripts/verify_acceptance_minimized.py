#!/usr/bin/env python3
"""
Full acceptance verify for aliceemoce fork.
- Launch Cockpit, minimize immediately (no mouse).
- Briefly restore only for capture, then minimize again.
- Navigate to Cursor via sidebar Invoke (no import modal).
- Check badge bundle, account audit, sort label, QuickSettings entry.
"""
from __future__ import annotations

import json
import subprocess
import time
from pathlib import Path

import uiautomation as auto

EXE = Path(
    r"C:\Users\aliceemoce\dev\cargo-target\cockpit-tools\release\cockpit-tools.exe"
)
INSTALLED_EXE = Path.home() / "AppData/Local/Cockpit Tools/cockpit-tools.exe"
REPO = Path(r"C:\Users\aliceemoce\dev\cockpit-tools")
OUT = Path(r"C:\Users\aliceemoce\AppData\Local\Temp")
REPORT = REPO / "scripts" / "acceptance_verify_report.json"
PNG_CURSOR = OUT / "acceptance-cursor-page.png"
PNG_QUICK = OUT / "acceptance-quicksettings.png"


def find_main_window() -> auto.Control | None:
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
            if width < 200 or height < 200:
                continue
            area = width * height
            if area > best_area:
                best_area = area
                best = w
        except Exception:
            pass
    return best


def wait_main_window(timeout_s: float = 25.0) -> auto.Control | None:
    deadline = time.time() + timeout_s
    while time.time() < deadline:
        win = find_main_window()
        if win:
            return win
        time.sleep(0.4)
    return None


def window_state(win: auto.Control) -> str:
    p = win.GetWindowPattern()
    if not p:
        return "unknown"
    s = p.WindowVisualState
    if s == auto.WindowVisualState.Minimized:
        return "minimized"
    if s == auto.WindowVisualState.Maximized:
        return "maximized"
    return "normal"


def minimize_invoke(win: auto.Control) -> bool:
    for ctrl in walk(win):
        try:
            if (ctrl.Name or "").strip() == "最小化" and ctrl.ControlTypeName == "ButtonControl":
                inv = ctrl.GetInvokePattern()
                if inv:
                    inv.Invoke()
                    time.sleep(0.35)
                    return window_state(win) == "minimized"
        except Exception:
            pass
    p = win.GetWindowPattern()
    if p:
        p.SetWindowVisualState(auto.WindowVisualState.Minimized)
        time.sleep(0.35)
        return window_state(win) == "minimized"
    return False


def restore_normal(win: auto.Control) -> None:
    p = win.GetWindowPattern()
    if p:
        p.SetWindowVisualState(auto.WindowVisualState.Normal)
        time.sleep(0.5)


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


def invoke_cursor_sidebar(win: auto.Control) -> bool:
    dismiss_blocking_modals(win)
    for ctrl in walk(win):
        try:
            n = (ctrl.Name or "").strip()
            if ctrl.ControlTypeName != "ButtonControl":
                continue
            if n == "Cursor" or n.startswith("Cursor "):
                inv = ctrl.GetInvokePattern()
                if inv:
                    inv.Invoke()
                    time.sleep(1.2)
                    return True
        except Exception:
            pass
    return invoke_by_name(win, "Cursor", partial=False)


def dismiss_blocking_modals(win: auto.Control) -> None:
    for label in ("关闭", "取消", "知道了", "确定"):
        if invoke_by_name(win, label, partial=False):
            time.sleep(0.6)
            break


def invoke_by_name(win: auto.Control, name: str, partial: bool = False) -> bool:
    for ctrl in walk(win):
        try:
            n = (ctrl.Name or "").strip()
            if not n:
                continue
            if (name == n) or (partial and name in n):
                inv = ctrl.GetInvokePattern()
                if inv:
                    inv.Invoke()
                    time.sleep(0.8)
                    return True
        except Exception:
            pass
    return False


def collect_names(win: auto.Control) -> list[str]:
    names: list[str] = []
    for ctrl in walk(win):
        try:
            n = (ctrl.Name or "").strip()
            if n:
                names.append(n)
        except Exception:
            pass
    return names




def find_cockpit_hwnd() -> int:
    import win32gui

    matches: list[int] = []

    def callback(hwnd, _extra) -> bool:
        if not win32gui.IsWindowVisible(hwnd):
            return True
        title = win32gui.GetWindowText(hwnd) or ""
        if "Cockpit" in title and "Tools" in title:
            matches.append(hwnd)
        return True

    win32gui.EnumWindows(callback, None)
    return matches[0] if matches else 0


def capture_hwnd(win: auto.Control, path: Path) -> None:
    import win32gui
    from PIL import ImageGrab

    hwnd = int(win.NativeWindowHandle or 0) or find_cockpit_hwnd()
    if not hwnd:
        raise RuntimeError("no hwnd")
    win32gui.ShowWindow(hwnd, 9)
    win32gui.SetForegroundWindow(hwnd)
    time.sleep(0.4)
    rect = win32gui.GetWindowRect(hwnd)
    ImageGrab.grab(rect).save(path)


def ensure_visible_window(win: auto.Control) -> auto.Control:
    fresh = wait_main_window(8.0) or find_main_window() or win
    p = fresh.GetWindowPattern()
    if p:
        p.SetWindowVisualState(auto.WindowVisualState.Normal)
        time.sleep(0.5)
        try:
            r = fresh.BoundingRectangle
            if r.width() < 800 or r.height() < 600:
                p.SetWindowVisualState(auto.WindowVisualState.Maximized)
                time.sleep(0.6)
        except Exception:
            pass
    return wait_main_window(3.0) or find_main_window() or fresh


def capture(win: auto.Control, path: Path) -> None:
    last_err: Exception | None = None
    for _ in range(4):
        fresh = ensure_visible_window(win)
        time.sleep(0.8)
        fresh = wait_main_window(3.0) or find_main_window() or fresh
        try:
            fresh.SetFocus()
        except Exception:
            pass
        try:
            fresh.CaptureToImage(str(path))
            minimize_invoke(fresh)
            return
        except Exception as exc:
            last_err = exc
            try:
                capture_hwnd(fresh, path)
                minimize_invoke(fresh)
                return
            except Exception as exc2:
                last_err = exc2
            time.sleep(0.6)
    raise last_err or RuntimeError("capture failed")


def run_audit() -> dict:
    script = REPO / "scripts" / "audit_cursor_accounts.py"
    proc = subprocess.run(
        ["python", str(script)],
        capture_output=True,
        text=True,
        timeout=120,
    )
    try:
        return json.loads(proc.stdout.strip().splitlines()[-1])
    except Exception:
        return {"error": proc.stdout[-500:] + proc.stderr[-500:]}


def bundle_has_badge() -> bool:
    for path in (REPO / "dist" / "assets").glob("PlatformOverviewTabsHeader*.js"):
        if "installed-version-badge" in path.read_text(encoding="utf-8", errors="ignore"):
            return True
    return False


def resolve_launch_exe() -> Path:
  # 优先 release（自动化环境对 LocalAppData 路径偶发 PermissionError）
    if EXE.is_file():
        return EXE
    return INSTALLED_EXE


def main() -> None:
    report: dict = {"steps": []}
    launch_exe = resolve_launch_exe()
    report["launch_exe"] = str(launch_exe)

    subprocess.run(["taskkill", "/F", "/IM", "cockpit-tools.exe"], capture_output=True)
    time.sleep(2)
    subprocess.Popen([str(launch_exe)], cwd=launch_exe.parent)
    win = wait_main_window(50.0)
    if not win:
        report["ok"] = False
        report["error"] = "no_window"
        REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
        print(json.dumps(report, ensure_ascii=False))
        return

    time.sleep(12)
    win = ensure_visible_window(wait_main_window(15.0) or win)
    dismiss_blocking_modals(win)

    nav_ok = False
    for attempt in range(6):
        win = ensure_visible_window(wait_main_window(5.0) or win)
        dismiss_blocking_modals(win)
        if invoke_cursor_sidebar(win):
            nav_ok = True
            break
        time.sleep(1.5)
    report["steps"].append({"navigate_cursor_sidebar": nav_ok})
    time.sleep(3)
    win = ensure_visible_window(wait_main_window(5.0) or win)

    names = collect_names(win)
    keywords = ["检测", "未检测", "Cursor.exe", "按剩余 Credits", "Cursor 设置", "ERR_CONNECTION"]
    hits = {k: [n for n in names if k in n][:8] for k in keywords}
    capture(win, PNG_CURSOR)
    report["cursor_page"] = {
        "png": str(PNG_CURSOR),
        "keyword_hits": hits,
        "has_credits_sort": any("Credits" in n for n in names),
        "has_cursor_settings": any("Cursor 设置" in n for n in names),
        "has_badge_text": bool(hits["检测"] or hits["未检测"] or hits["Cursor.exe"]),
        "connection_error": bool(hits["ERR_CONNECTION"]),
    }

    restore_normal(win)
    time.sleep(0.4)
    qs_open = invoke_by_name(win, "Cursor 设置", partial=False)
    time.sleep(1.2)
    qs_names = collect_names(win)
    qs_hits = {
        "Download": [n for n in qs_names if "Download" in n or "下载" in n][:5],
        "path": [n for n in qs_names if "路径" in n or "path" in n.lower()][:5],
        "RefreshCw": [n for n in qs_names if "刷新" in n][:8],
    }
    qs_capture_ok = True
    try:
        capture(win, PNG_QUICK)
    except Exception as exc:
        qs_capture_ok = False
        try:
            capture_hwnd(win, PNG_QUICK)
        except Exception:
            pass
        report.setdefault("errors", []).append({"quick_settings_capture": str(exc)})
    minimize_invoke(find_main_window() or win)
    report["quick_settings"] = {
        "opened": qs_open,
        "capture_ok": qs_capture_ok,
        "png": str(PNG_QUICK),
        "hits": qs_hits,
    }

    report["account_audit"] = run_audit()
    report["bundle_badge"] = bundle_has_badge()
    report["exe"] = str(EXE)
    report["exe_exists"] = EXE.is_file()
    report["installed_exe"] = str(INSTALLED_EXE)
    installed_deployed = False
    if EXE.is_file() and INSTALLED_EXE.is_file():
        rel = EXE.stat()
        ins = INSTALLED_EXE.stat()
        report["installed_exe_size"] = ins.st_size
        report["release_exe_size"] = rel.st_size
        report["installed_exe_mtime"] = ins.st_mtime
        report["release_exe_mtime"] = rel.st_mtime
        installed_deployed = ins.st_size == rel.st_size and ins.st_mtime >= rel.st_mtime - 2
    report["installed_matches_release"] = installed_deployed
    report["ok"] = (
        nav_ok
        and report["cursor_page"]["has_cursor_settings"]
        and report["cursor_page"]["has_credits_sort"]
        and not report["cursor_page"]["connection_error"]
        and report["bundle_badge"]
        and report["account_audit"].get("missing_detail_count", 1) == 0
        and installed_deployed
    )
    minimized = minimize_invoke(find_main_window() or win)
    report["steps"].append({"minimize_after_verify": minimized})
    report["minimized_after"] = window_state(find_main_window() or win) == "minimized"

    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False))


if __name__ == "__main__":
    main()
