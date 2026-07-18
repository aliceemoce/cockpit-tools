#!/usr/bin/env python3
"""Batch-prepare and probe Cursor accounts via isolated agent CLI.

Writes only classification outcomes (no tokens) to a results JSON.
"""
from __future__ import annotations

import base64
import json
import os
import re
import subprocess
import sys
import time
from pathlib import Path

from cryptography.hazmat.primitives.ciphers.aead import AESGCM

ROOT = Path(r"C:\Users\aliceemoce\.antigravity_cockpit")
KEY_PATH = ROOT / "secure-account-storage.key"
INDEX_PATH = ROOT / "cursor_accounts.json"
ACC_DIR = ROOT / "cursor_accounts"
PROBE_ROOT = Path(r"C:\Users\aliceemoce\AppData\Local\Temp\cursor-agent-probe-batch")
RESULTS = PROBE_ROOT / "results.jsonl"
AGENT = r"C:\Users\aliceemoce\AppData\Local\cursor-agent\agent.ps1"


def decrypt_account(account_id: str) -> dict:
    key = base64.b64decode(KEY_PATH.read_text(encoding="utf-8").strip())
    path = ACC_DIR / f"{account_id}.json"
    env = json.loads(path.read_text(encoding="utf-8"))
    nonce = base64.b64decode(env["nonce"])
    ct = base64.b64decode(env["ciphertext"])
    pt = AESGCM(key).decrypt(nonce, ct, None)
    return json.loads(pt)


def classify(stdout: str, stderr: str, exit_code: int) -> str:
    text = f"{stdout}\n{stderr}".lower()
    if "usage limit" in text or "rate_limited" in text or "free requests limit" in text:
        return "rate_limited"
    if "unauthorized" in text or "not authenticated" in text or "authentication" in text and "fail" in text:
        return "auth_failed"
    if "network" in text or "econn" in text or "timeout" in text or "fetch failed" in text:
        return "network_error"
    if exit_code == 0 and stdout.strip():
        return "ok"
    if exit_code == 0:
        return "ok_empty"
    return "unknown_error"


def pick_candidates(limit: int) -> list[dict]:
    idx = json.loads(INDEX_PATH.read_text(encoding="utf-8"))
    # Prefer accounts with no quota error and free membership in index
    cands = []
    for s in idx["accounts"]:
        if s.get("quota_query_last_error"):
            continue
        cands.append(s)
        if len(cands) >= limit * 5:
            break
    # Load detail for first N that have tokens and prefer lower usage %
    scored = []
    for s in cands:
        try:
            acc = decrypt_account(s["id"])
        except Exception:
            continue
        if not acc.get("access_token"):
            continue
        usage = acc.get("cursor_usage_raw") or {}
        plan = ((usage.get("individualUsage") or {}).get("plan") or {})
        total = plan.get("totalPercentUsed")
        auto = plan.get("autoPercentUsed")
        score = 999.0
        vals = [v for v in (total, auto) if isinstance(v, (int, float))]
        if vals:
            score = max(vals)
        scored.append((score, acc))
    scored.sort(key=lambda x: x[0])
    return [a for _, a in scored[:limit]]


def probe_one(acc: dict) -> dict:
    work = PROBE_ROOT / acc["id"]
    (work / "Cursor").mkdir(parents=True, exist_ok=True)
    auth = {
        "accessToken": acc["access_token"],
        "refreshToken": acc.get("refresh_token") or acc["access_token"],
    }
    (work / "Cursor" / "auth.json").write_text(json.dumps(auth, indent=2), encoding="utf-8")
    usage = acc.get("cursor_usage_raw") or {}
    plan = ((usage.get("individualUsage") or {}).get("plan") or {})
    env = os.environ.copy()
    env["APPDATA"] = str(work)
    env["CURSOR_INVOKED_AS"] = "agent"
    cmd = [
        "powershell",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        AGENT,
        "status",
        "--format",
        "json",
    ]
    st = subprocess.run(cmd, capture_output=True, text=True, env=env, timeout=60)
    status_out = st.stdout
    status_err = st.stderr
    status_email = None
    try:
        status_email = json.loads(status_out).get("userInfo", {}).get("email")
    except Exception:
        pass

    chat = [
        "powershell",
        "-NoProfile",
        "-ExecutionPolicy",
        "Bypass",
        "-File",
        AGENT,
        "-p",
        "--mode",
        "ask",
        "--output-format",
        "json",
        "--trust",
        "--workspace",
        str(work),
        "Reply with exactly one word: pong",
    ]
    t0 = time.time()
    ch = subprocess.run(chat, capture_output=True, text=True, env=env, timeout=120)
    elapsed_ms = int((time.time() - t0) * 1000)
    outcome = classify(ch.stdout, ch.stderr, ch.returncode)
    # Truncate bodies; never include tokens
    def clip(s: str, n: int = 800) -> str:
        s = s or ""
        return s[:n]

    return {
        "id": acc.get("id"),
        "email": acc.get("email"),
        "auth_id": acc.get("auth_id"),
        "membership_type": acc.get("membership_type"),
        "usage_total_pct": plan.get("totalPercentUsed"),
        "usage_auto_pct": plan.get("autoPercentUsed"),
        "breakdown_total": ((plan.get("breakdown") or {}).get("total")),
        "status_exit": st.returncode,
        "status_email": status_email,
        "chat_exit": ch.returncode,
        "chat_outcome": outcome,
        "elapsed_ms": elapsed_ms,
        "stdout_clip": clip(ch.stdout),
        "stderr_clip": clip(ch.stderr),
        "status_stderr_clip": clip(status_err),
    }


def main() -> int:
    limit = int(sys.argv[1]) if len(sys.argv) > 1 else 5
    PROBE_ROOT.mkdir(parents=True, exist_ok=True)
    accounts = pick_candidates(limit)
    print(f"candidates={len(accounts)}", flush=True)
    with RESULTS.open("w", encoding="utf-8") as f:
        for i, acc in enumerate(accounts, 1):
            print(f"[{i}/{len(accounts)}] probing {acc.get('email')} ...", flush=True)
            try:
                row = probe_one(acc)
            except Exception as e:
                row = {
                    "id": acc.get("id"),
                    "email": acc.get("email"),
                    "chat_outcome": "probe_exception",
                    "error": str(e)[:400],
                }
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
            f.flush()
            print(
                f"  -> {row.get('chat_outcome')} usage_total={row.get('usage_total_pct')} "
                f"status_email={row.get('status_email')}",
                flush=True,
            )
    print(f"results={RESULTS}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
