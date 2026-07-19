#!/usr/bin/env python3
"""Sampling check: percent scale (aligned with upstream + disk audit).

Evidence (disk audit 2026-07-17):
- FREE accounts: breakdown.total ≈ used amount; total/pct often implies cap≈200
- breakdown.total==0 means unused so far, NOT "no quota"
- Reliable remaining signal: totalPercentUsed < 100

Buckets:
- usable: has usage_raw, no quota_query_last_error, totalPercentUsed < 100
- plan_exhausted: totalPercentUsed >= 100
- quota_query_failed / no_usage_raw / unknown
"""
from __future__ import annotations

import base64
import json
import random
import sys
from pathlib import Path

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(r"C:\Users\aliceemoce\.antigravity_cockpit")
KEY_PATH = ROOT / "secure-account-storage.key"
INDEX_PATH = ROOT / "cursor_accounts.json"
ACC_DIR = ROOT / "cursor_accounts"
OUT = Path(r"C:\Users\aliceemoce\AppData\Local\Temp\cursor-quota-sample.json")


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
    elif isinstance(pct, (int, float)) and pct >= 100:
        bucket = "plan_exhausted"
    elif isinstance(pct, (int, float)) and pct < 100:
        bucket = "usable"
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


def main() -> int:
    idx = json.loads(INDEX_PATH.read_text(encoding="utf-8"))
    ids = [s["id"] for s in (idx.get("accounts") or [])]
    random.seed(20260717)
    random.shuffle(ids)
    scan_limit = min(len(ids), int(sys.argv[1]) if len(sys.argv) > 1 else 400)
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
    usable = buckets.get("usable") or []
    report = {
        "standard": {
            "usable": "has usage_raw AND no quota_query_last_error AND totalPercentUsed<100",
            "note": "breakdown.total is cumulative used, not capacity; total==0 is unused not dead",
        },
        "scan": {"scanned": scan_limit, "counts": counts},
        "sample_usable": usable[:12],
        "sample_exhausted": (buckets.get("plan_exhausted") or [])[:8],
        "sample_zero_breakdown_total_but_usable": [
            r for r in usable if r.get("breakdown_total") == 0
        ][:8],
    }
    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps(counts, ensure_ascii=False))
    print("wrote", OUT)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
