#!/usr/bin/env python3
"""从账号库选两枚 token 完整且邮箱不同的账号，写回 cursor_instances.json。"""
from __future__ import annotations

import json
from pathlib import Path

ACCOUNTS_DIR = Path.home() / ".antigravity_cockpit" / "cursor_accounts"
INSTANCES_JSON = Path.home() / ".antigravity_cockpit" / "cursor_instances.json"
INVALID_MARKERS = ("refresh token 已失效", "会话已过期或未认证")


def account_score(path: Path) -> dict | None:
    try:
        data = json.loads(path.read_text(encoding="utf-8"))
    except (OSError, json.JSONDecodeError):
        return None
    email = (data.get("email") or "").strip()
    access = (data.get("access_token") or "").strip()
    refresh = (data.get("refresh_token") or "").strip()
    if not email or not access or not refresh:
        return None
    err = (data.get("quota_query_last_error") or "").strip()
    if any(m in err for m in INVALID_MARKERS):
        return None
    return {
        "id": data.get("id") or path.stem,
        "email": email,
        "last_used": int(data.get("last_used") or 0),
        "path": str(path),
    }


def main() -> None:
    candidates: list[dict] = []
    for path in sorted(ACCOUNTS_DIR.glob("cursor_*.json")):
        row = account_score(path)
        if row:
            candidates.append(row)
    candidates.sort(key=lambda x: x["last_used"], reverse=True)
    if len(candidates) < 2:
        raise SystemExit(f"可用账号不足 2 个，仅找到 {len(candidates)} 个")

    default_pick = candidates[0]
    multi_pick = next(c for c in candidates[1:] if c["email"].lower() != default_pick["email"].lower())

    store = json.loads(INSTANCES_JSON.read_text(encoding="utf-8"))
    store.setdefault("defaultSettings", {})["bindAccountId"] = default_pick["id"]
    if store.get("instances"):
        inst = store["instances"][0]
        inst["bindAccountId"] = multi_pick["id"]
        inst["workingDir"] = str(Path.home() / "dev" / "cockpit-tools")
    INSTANCES_JSON.write_text(json.dumps(store, ensure_ascii=False, indent=2), encoding="utf-8")

    out = {
        "default": default_pick,
        "multi": multi_pick,
        "instances_json": str(INSTANCES_JSON),
    }
    print(json.dumps(out, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
