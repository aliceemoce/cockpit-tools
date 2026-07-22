#!/usr/bin/env python3
"""Prepare isolated Cursor agent auth for one cockpit account (no token stdout)."""
from __future__ import annotations

import base64
import json
import sys
from pathlib import Path

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(r"C:\Users\aliceemoce\.antigravity_cockpit")
KEY_PATH = ROOT / "secure-account-storage.key"
INDEX_PATH = ROOT / "cursor_accounts.json"
ACC_DIR = ROOT / "cursor_accounts"
PROBE = Path(r"C:\Users\aliceemoce\AppData\Local\Temp\cursor-agent-probe-isolated")
TARGET_AUTH = "user_01KWDB1MSJ96M2FWECJTSPR59W"


def decrypt_account(account_id: str) -> dict:
    key = base64.b64decode(KEY_PATH.read_text(encoding="utf-8").strip())
    path = ACC_DIR / f"{account_id}.json"
    env = json.loads(path.read_text(encoding="utf-8"))
    if env.get("algorithm") != "AES-256-GCM":
        raise SystemExit(f"unexpected envelope: {path}")
    nonce = base64.b64decode(env["nonce"])
    ct = base64.b64decode(env["ciphertext"])
    pt = AESGCM(key).decrypt(nonce, ct, None)
    return json.loads(pt)


def main() -> int:
    idx = json.loads(INDEX_PATH.read_text(encoding="utf-8"))
    picked = None
    for s in idx["accounts"]:
        aid = s.get("auth_id") or ""
        if TARGET_AUTH in aid or aid.endswith(TARGET_AUTH):
            picked = s
            break
    if picked is None:
        for s in idx["accounts"]:
            if (s.get("membership_type") or "").lower() == "free":
                picked = s
                break
    if picked is None:
        print("no account found", file=sys.stderr)
        return 1

    acc = decrypt_account(picked["id"])
    usage = acc.get("cursor_usage_raw") or {}
    plan = ((usage.get("individualUsage") or {}).get("plan") or {})
    meta = {
        "id": acc.get("id"),
        "email": acc.get("email"),
        "auth_id": acc.get("auth_id"),
        "membership_type": acc.get("membership_type"),
        "quota_query_last_error": acc.get("quota_query_last_error"),
        "usage_updated_at": acc.get("usage_updated_at"),
        "has_access": bool(acc.get("access_token")),
        "has_refresh": bool(acc.get("refresh_token")),
        "access_len": len(acc.get("access_token") or ""),
        "refresh_len": len(acc.get("refresh_token") or ""),
        "totalPercentUsed": plan.get("totalPercentUsed"),
        "autoPercentUsed": plan.get("autoPercentUsed"),
        "breakdown_total": ((plan.get("breakdown") or {}).get("total")),
    }
    print(json.dumps(meta, ensure_ascii=False, indent=2))

    (PROBE / "Cursor").mkdir(parents=True, exist_ok=True)
    auth = {
        "accessToken": acc["access_token"],
        "refreshToken": acc.get("refresh_token") or acc["access_token"],
    }
    (PROBE / "Cursor" / "auth.json").write_text(
        json.dumps(auth, indent=2), encoding="utf-8"
    )
    (PROBE / "meta.json").write_text(
        json.dumps(meta, indent=2, ensure_ascii=False), encoding="utf-8"
    )
    print(f"probe_auth_written={PROBE / 'Cursor' / 'auth.json'}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
