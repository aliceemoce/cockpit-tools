#!/usr/bin/env python3
"""Verify Cursor display labels match Agent chat probe outcomes for sampled accounts."""
from __future__ import annotations

import base64
import json
import re
import sys
from pathlib import Path

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(r"C:\Users\aliceemoce\.antigravity_cockpit")
KEY = base64.b64decode((ROOT / "secure-account-storage.key").read_text(encoding="utf-8").strip())
ACC = ROOT / "cursor_accounts"
SAMPLE = Path(r"C:\Users\aliceemoce\AppData\Local\Temp\cursor-quota-sample.json")
OUT = Path(r"C:\Users\aliceemoce\AppData\Local\Temp\cursor-display-chat-alignment.json")


def dec(aid: str) -> dict:
    env = json.loads((ACC / f"{aid}.json").read_text(encoding="utf-8"))
    pt = AESGCM(KEY).decrypt(
        base64.b64decode(env["nonce"]), base64.b64decode(env["ciphertext"]), None
    )
    return json.loads(pt)


def display_label(acc: dict) -> str:
    probe = (acc.get("chat_probe") or {}).get("outcome")
    if probe == "ok":
        return "有剩余"
    if probe == "rate_limited":
        return "额度用尽"
    usage = acc.get("cursor_usage_raw") or {}
    plan = ((usage.get("individualUsage") or {}).get("plan") or {})
    pct = plan.get("totalPercentUsed")
    if pct is not None and pct >= 100:
        return "待验活"
    if acc.get("quota_query_last_error"):
        return "配额查询失败"
    if acc.get("cursor_usage_raw") is None:
        return "配额未查询"
    return "有剩余"


def main() -> int:
  # mirror TS resolveCursorQuotaAvailability (1.3.11)
    data = json.loads(SAMPLE.read_text(encoding="utf-8"))
    rows = []
    mismatches = []
    for bucket_key in ("sample_usable", "sample_exhausted"):
        for item in data.get(bucket_key) or []:
            aid = item["id"]
            acc = dec(aid)
            probe = (acc.get("chat_probe") or {}).get("outcome")
            detail = (acc.get("chat_probe") or {}).get("detail") or ""
            m = re.search(r'"result":"([^"]+)"', detail)
            reply = m.group(1) if m else (detail[:120] if detail else None)
            label = display_label(acc)
            if probe == "ok":
                aligned = label == "有剩余"
            elif probe == "rate_limited":
                aligned = label == "额度用尽"
            elif item.get("bucket") == "usable":
                aligned = label == "有剩余"
            else:
                aligned = label in ("待验活", "额度用尽")
            row = {
                "id": aid,
                "email": item.get("email"),
                "disk_bucket": item.get("bucket"),
                "totalPercentUsed": item.get("totalPercentUsed"),
                "chat_probe": probe,
                "reply_snippet": reply,
                "display_label": label,
                "aligned": aligned,
            }
            rows.append(row)
            if probe and not aligned:
                mismatches.append(row)
    report = {
        "version": "1.3.11",
        "method": "Agent CLI open prompt (cursor_chat_probe), not usage-summary alone",
        "sample_count": len(rows),
        "aligned_count": sum(1 for r in rows if r["aligned"]),
        "mismatch_count": len(mismatches),
        "usable_sample_chat_ok": sum(
            1 for r in rows if r["disk_bucket"] == "usable" and r["chat_probe"] == "ok"
        ),
        "exhausted_disk_rate_limited": sum(
            1
            for r in rows
            if r["disk_bucket"] == "plan_exhausted" and r["chat_probe"] == "rate_limited"
        ),
        "exhausted_disk_still_chats": sum(
            1
            for r in rows
            if r["disk_bucket"] == "plan_exhausted" and r["chat_probe"] == "ok"
        ),
        "rows": rows,
    }
    OUT.write_text(json.dumps(report, ensure_ascii=False, indent=2), encoding="utf-8")
    print(json.dumps({k: report[k] for k in report if k != "rows"}, ensure_ascii=False))
    print("wrote", OUT)
    return 0 if report["mismatch_count"] == 0 else 1


if __name__ == "__main__":
    raise SystemExit(main())
