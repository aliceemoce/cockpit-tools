#!/usr/bin/env python3
"""
纯后端验证（无 UI、无鼠标）：
1. 静态：总览与多开是否同一 Rust 入口
2. 对比 nirvana 推断流程 vs cockpit switch 步骤
3. 可选：对 Cursor profile 做 dry-run 文件键检查（不启动 GUI）
"""
from __future__ import annotations

import json
import os
import sqlite3
import subprocess
import sys
from pathlib import Path

REPO = Path(r"C:\Users\aliceemoce\dev\cockpit-tools")
REPORT = REPO / "scripts" / "backend_verify_report.json"
NIRVANA_REPORT = REPO / "scripts" / "nirvana_cursor_logic_report.json"
NIRVANA_FLOW = REPO / "scripts" / "nirvana_cursor_flow.json"
COMPARE_REPORT = REPO / "scripts" / "three_way_compare_report.json"

COCKPIT_SWITCH_STEPS = [
    "close_cursor",
    "hard_reset_cursor_fingerprint_state_for_profile",
    "reset_windows_machine_guid (optional)",
    "patch_cursor_workbench_auth_bridge",
    "inject_account_to_profile (cursorAuth/* aligned with nirvana writeTokenToDb)",
    "cursor_start_instance_prepared",
]


def run_py(script: Path) -> tuple[int, str]:
    p = subprocess.run([sys.executable, str(script)], capture_output=True, text=True, encoding="utf-8", errors="replace")
    return p.returncode, (p.stdout or "") + (p.stderr or "")


def static_paths() -> dict:
    p = subprocess.run(
        [sys.executable, str(REPO / "scripts" / "verify_cursor_switch_paths.py")],
        capture_output=True,
        text=True,
    )
    return {"exit": p.returncode, "output": (p.stdout or p.stderr or "").strip()}


def read_json(p: Path) -> dict:
    if p.is_file():
        return json.loads(p.read_text(encoding="utf-8"))
    return {}


def cursor_profile_paths() -> dict:
    appdata = os.environ.get("APPDATA", "")
    base = Path(appdata) / "Cursor"
    storage = base / "User" / "globalStorage" / "storage.json"
    machine = base / "machineId"
    vscdb = base / "User" / "globalStorage" / "state.vscdb"
    return {
        "profile_dir": str(base),
        "storage_json": str(storage),
        "machine_id": str(machine),
        "state_vscdb": str(vscdb),
        "storage_exists": storage.is_file(),
        "machine_exists": machine.is_file(),
        "vscdb_exists": vscdb.is_file(),
    }


def read_vscdb_token(db_path: Path) -> str | None:
    if not db_path.is_file():
        return None
    conn = sqlite3.connect(f"file:{db_path}?mode=ro", uri=True)
    try:
        row = conn.execute(
            "SELECT value FROM ItemTable WHERE key = 'cursorAuth/accessToken' LIMIT 1"
        ).fetchone()
        if row and row[0]:
            t = str(row[0]).strip()
            return t[:12] + "..." if len(t) > 12 else t
    finally:
        conn.close()
    return None


def read_storage_telemetry(storage: Path) -> dict:
    if not storage.is_file():
        return {}
    try:
        obj = json.loads(storage.read_text(encoding="utf-8"))
    except Exception:
        return {}
    keys = [
        "telemetry.machineId",
        "telemetry.macMachineId",
        "telemetry.devDeviceId",
        "telemetry.sqmId",
    ]
    out = {}
    for k in keys:
        v = obj.get(k)
        if isinstance(v, str) and v:
            out[k] = v[:12] + "..."
    return out


def compare_flows(nirvana: dict, nirvana_scan: dict) -> dict:
    n_steps = nirvana.get("ordered_steps_inferred") or nirvana_scan.get("inferred_steps") or []
    # map nirvana inferred to cockpit ops
    mapping = []
    for step in n_steps:
        low = step.lower()
        if "关闭" in step or "kill" in low or "close" in low:
            mapping.append({"nirvana": step, "cockpit": "close_cursor", "match": True})
        elif "patch" in low or "auth bridge" in low or "workbench" in low:
            mapping.append({"nirvana": step, "cockpit": "patch_cursor_workbench_auth_bridge", "match": True})
        elif "machineguid" in low.replace(" ", ""):
            mapping.append({"nirvana": step, "cockpit": "reset_windows_machine_guid", "match": True})
        elif "telemetry" in low or "machineid" in low or "fingerprint" in low or "storage.json" in low or "vscdb" in low:
            mapping.append({"nirvana": step, "cockpit": "hard_reset_*", "match": True})
        elif "token" in low or "switchaccount" in low.replace(" ", ""):
            mapping.append({"nirvana": step, "cockpit": "inject_account_to_profile", "match": True})
        elif "启动" in step or "launch" in low:
            mapping.append({"nirvana": step, "cockpit": "cursor_start_instance_prepared", "match": True})
        else:
            mapping.append({"nirvana": step, "cockpit": "?", "match": False})
    return {"nirvana_steps": n_steps, "cockpit_steps": COCKPIT_SWITCH_STEPS, "mapping": mapping}


def main() -> int:
    errors: list[str] = []

    # ensure nirvana report exists
    run_py(REPO / "scripts" / "scan_nirvana_installed.py")
    run_py(REPO / "scripts" / "extract_nirvana_cursor_flow.py")

    if not COMPARE_REPORT.is_file():
        run_py(REPO / "scripts" / "three_way_compare.py")

    static = static_paths()
    if static["exit"] != 0:
        errors.append(f"verify_cursor_switch_paths failed: {static['output']}")

    nirvana = read_json(NIRVANA_FLOW) or read_json(NIRVANA_REPORT)
    nirvana_scan = read_json(NIRVANA_REPORT)
    three = read_json(COMPARE_REPORT)
    paths = cursor_profile_paths()
    token_preview = read_vscdb_token(Path(paths["state_vscdb"]))
    telemetry_preview = read_storage_telemetry(Path(paths["storage_json"]))

    flow_cmp = compare_flows(nirvana, nirvana_scan)
    mapping = flow_cmp.get("mapping", [])
    flow_ok = len(mapping) > 0 and all(m.get("match") for m in mapping)
    hit_count = nirvana_scan.get("hit_count") or len(nirvana_scan.get("matches", []))
    if hit_count == 0 and not nirvana.get("flows"):
        errors.append("nirvana: no cursor-related JS hits extracted")
        flow_ok = False
    # nirvana 额外有 workbench patch + Windows MachineGuid，cockpit 当前未实现
    nirvana_extra = nirvana.get("flows") or []
    has_workbench_patch = any(f.get("marker") == "patchCursorWorkbench" for f in nirvana_extra)
    align_rs = REPO / "src-tauri/src/modules/cursor_switch_align.rs"
    cockpit_has_align = align_rs.is_file() and "patch_cursor_workbench_auth_bridge" in align_rs.read_text(encoding="utf-8")
    if has_workbench_patch and not cockpit_has_align:
        flow_cmp["cockpit_gap"] = ["cursor_switch_align 模块缺失"]

    report = {
        "static_switch_paths": static,
        "three_way_local_unified": three.get("local_unified"),
        "cockpit_has_assistant_strings": three.get("cockpit_has_assistant_strings"),
        "nirvana_extracted": nirvana_scan.get("exists"),
        "nirvana_hit_count": hit_count,
        "nirvana_has_workbench_patch": has_workbench_patch if nirvana_extra else False,
        "cockpit_has_switch_align_module": cockpit_has_align if nirvana_extra else False,
        "flow_comparison": flow_cmp,
        "flow_matches_nirvana": flow_ok,
        "cursor_profile": paths,
        "cursor_token_preview": token_preview,
        "cursor_telemetry_preview": telemetry_preview,
        "errors": errors,
        "ok": static["exit"] == 0 and three.get("local_unified") is True and flow_ok and hit_count > 0,
    }
    REPORT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(report, ensure_ascii=False, indent=2))
    return 0 if report["ok"] else 1


if __name__ == "__main__":
    sys.exit(main())
