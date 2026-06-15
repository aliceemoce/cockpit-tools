#!/usr/bin/env python3
"""Clear transient (network) quota_query_last_error from Cursor account JSON files."""

from __future__ import annotations

import json
from collections import Counter
from pathlib import Path

TRANSIENT_MARKERS = (
    "error sending request",
    "timed out",
    "timeout",
    "connection",
    "connect error",
    "dns",
    "resolve",
    "proxy",
    "tunnel",
    "502",
    "503",
    "504",
)


def is_transient_error(message: str) -> bool:
    lower = message.lower()
    return any(marker in lower for marker in TRANSIENT_MARKERS)


def main() -> int:
    root = Path.home() / ".antigravity_cockpit" / "cursor_accounts"
    if not root.is_dir():
        print(f"目录不存在: {root}")
        return 1

    total = 0
    with_error = 0
    cleared = 0
    kept = Counter()

    for path in sorted(root.glob("cursor_*.json")):
        total += 1
        data = json.loads(path.read_text(encoding="utf-8"))
        err = (data.get("quota_query_last_error") or "").strip()
        if not err:
            continue
        with_error += 1
        if is_transient_error(err):
            data["quota_query_last_error"] = None
            data["quota_query_last_error_at"] = None
            path.write_text(
                json.dumps(data, ensure_ascii=False, indent=2) + "\n",
                encoding="utf-8",
            )
            cleared += 1
        else:
            key = err[:60]
            kept[key] += 1

    print(f"total_accounts={total}")
    print(f"with_error_before={with_error}")
    print(f"cleared_transient={cleared}")
    print(f"kept_non_transient={sum(kept.values())}")
    for key, count in kept.most_common(10):
        print(f"  kept[{count}] {key}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
