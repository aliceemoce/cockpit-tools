#!/usr/bin/env python3
"""Sampling check: quota standard first, then optional Agent CLI on a small sample.

Standard (HR-20260717-003):
- breakdown.total == 0  => 无额度 / 不可用（即使 CLI ask 返回 pong 也不得标可用）
- totalPercentUsed >= 100 => 额度用光 / 不可用
- usable_candidate: total > 0 and totalPercentUsed < 100 and no quota_query_last_error

This script samples; it does not invent a product UI hard-standard.
"""
from __future__ import annotations

import base64
import json
import os
import random
import subprocess
import sys
import time
from pathlib import Path

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(r"C:\Users\aliceemoce\.antigravity_cockpit")
KEY_PATH = ROOT / "secure-account-storage.key"
INDEX_PATH = ROOT / "cursor_accounts.json"
ACC_DIR = ROOT / "cursor_accounts"
OUT = Path(r"C:\Users\aliceemoce\AppData\Local\Temp\cursor-quota-sample.json")
AGENT = r"C:\Users\aliceemoce\AppData\Local\cursor-agent\agent.ps1"
PROBE_EXE = Path(r"C:\Users\aliceemoce\dev\cockpit-tools\target\debug\cursor_chat_probe.exe")


def decrypt(account_id: str) -> dict:
    key = base64.b64decode(KEY_PATH.read_text(encoding="utf-8").strip())
    env = json.loads((ACC_DIR / f"{account_id}.json").read_text(encoding="utf-8"))
    pt = AESGCM(key).decrypt(base64.b64decode(env["nonce"]), base64.b64decode(env["ciphertext"]), None)
    return json.loads(pt)


def classify_quota(acc: dict) -> dict:
    usage = acc.get("cursor_usage_raw") or {}
    plan = ((usage.get("individualUsage") or {}).get("plan") or {})
    breakdown = plan.get("breakdown") or {}
    total = breakdown.get("total")
    if total is None:
        total = breakdown.get("Total")
    pct = plan.get("totalPercentUsed")
    auto = plan.get("autoPercentUsed")
    err = (acc.get("quota_query_last_error") or "").strip() or None
    if err:
        bucket = "quota_query_failed"
    elif acc.get("cursor_usage_raw") is None:
        bucket = "no_usage_raw"
    elif isinstance(total, (int, float)) and total == 0:
        bucket = "zero_plan_quota"
    elif isinstance(pct, (int, float)) and pct >= 100:
        bucket = "plan_exhausted"
    elif isinstance(total, (int, float)) and total > 0 and (
        pct is None or (isinstance(pct, (int, float)) and pct < 100)
    ):
        bucket = "usable_candidate"
    else:
        bucket = "unknown"
    return {
        "id": acc.get("id"),
        "email": acc.get("email"),
        "membership": acc.get("membership_type"),
        "breakdown_total": total,
        "totalPercentUsed": pct,
        "autoPercentUsed": auto,
        "quota_error": err,
        "bucket": bucket,
        "prior_chat_probe": (acc.get("chat_probe") or {}).get("outcome"),
    }


def probe_cli(account_id: str) -> dict:
    """Prefer already-built exe to avoid cargo (disk nearly full)."""
    if PROBE_EXE.is_file():
        p = subprocess.run(
            [str(PROBE_EXE), account_id],
            capture_output=True,
            text=True,
            timeout=180,
        )
        out = (p.stdout or "") + "\n" + (p.stderr or "")
        outcome = "unknown"
        for line in out.splitlines():
            if "outcome=" in line and line.strip().startswith("id="):
                # id=... outcome=ok ...
                for part in line.split():
                    if part.startswith("outcome="):
                        outcome = part.split("=", 1)[1]
        return {"exit": p.returncode, "outcome": outcome, "clip": out[-600:]}
    return {"exit": -1, "outcome": "probe_exe_missing", "clip": ""}


def main() -> int:
    sample_n = int(sys.argv[1]) if len(sys.argv) > 1 else 8
    cli_n = int(sys.argv[2]) if len(sys.argv) > 2 else 3
    idx = json.loads(INDEX_PATH.read_text(encoding="utf-8"))
    summaries = idx.get("accounts") or []
    # Load a capped random subset of detail files for classification (full 2000 decrypt is heavy)
    ids = [s["id"] for s in summaries]
    random.seed(20260717)
    random.shuffle(ids)
    scan_limit = min(len(ids), 400)
    buckets: dict[str, list] = {}
    classified = []
    for i, aid in enumerate(ids[:scan_limit]):
        try:
            row = classify_quota(decrypt(aid))
        except Exception as e:
            row = {"id": aid, "bucket": "decrypt_error", "error": str(e)[:200]}
        classified.append(row)
        buckets.setdefault(row["bucket"], []).append(row)
        if (i + 1) % 50 == 0:
            print(f"scanned {i+1}/{scan_limit}", flush=True)

    counts = {k: len(v) for k, v in sorted(buckets.items())}
    print("SCAN", json.dumps({"scanned": scan_limit, "counts": counts}, ensure_ascii=False))

    # Sampling for CLI: only from usable_candidate
    candidates = buckets.get("usable_candidate") or []
    zero = buckets.get("zero_plan_quota") or []
    sample_ok_pool = candidates[: max(sample_n, cli_n)]
    sample_zero = zero[: min(2, len(zero))]

    cli_targets = sample_ok_pool[:cli_n]
    # Also re-check one prior false-positive style zero-quota if present
    prior_false = [
        r
        for r in classified
        if r.get("email") in ("fated-eggs-9y@icloud.com", "never-shifts.0e@icloud.com")
    ]

    cli_results = []
    for row in cli_targets:
        print(f"CLI sample usable_candidate {row.get('email')} ...", flush=True)
        r = probe_cli(row["id"])
        cli_results.append({**row, "cli": r})
        print(f"  -> {r.get('outcome')}", flush=True)

    zero_cli = []
    for row in prior_false[:2]:
        print(f"CLI check zero/prior {row.get('email')} bucket={row.get('bucket')} ...", flush=True)
        r = probe_cli(row["id"])
        zero_cli.append({**row, "cli": r})
        print(f"  -> {r.get('outcome')} (quota_bucket={row.get('bucket')})", flush=True)

    report = {
        "standard": {
            "usable_candidate": "breakdown.total>0 AND totalPercentUsed<100 AND no quota_query_last_error",
            "not_usable": ["zero_plan_quota", "plan_exhausted", "quota_query_failed"],
            "note": "CLI pong alone must not override zero_plan_quota / plan_exhausted",
        },
        "scan": {"scanned": scan_limit, "counts": counts},
        "sample_usable_candidates": sample_ok_pool[:sample_n],
        "cli_on_usable_candidates": cli_results,
        "cli_on_prior_claimed_ok": zero_cli,
    }
    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print("WROTE", OUT)
    # Verdict lines
    for r in zero_cli:
        print(
            "VERDICT_PRIOR",
            r.get("email"),
            "bucket=",
            r.get("bucket"),
            "cli=",
            (r.get("cli") or {}).get("outcome"),
            "=>",
            "不可用(额度标准)" if r.get("bucket") in ("zero_plan_quota", "plan_exhausted") else "待定",
        )
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
