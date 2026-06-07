#!/usr/bin/env python3
"""Push remaining fork files to aliceemoce/cockpit-tools via GitHub Contents API."""
from __future__ import annotations

import base64
import json
import subprocess
import sys
import urllib.error
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OWNER = "aliceemoce"
REPO = "cockpit-tools"
BRANCH = "main"
MESSAGE = """fork: cursor refresh email, import backup sync, quick-settings install path

- Write API email to account.email on cursor refresh
- Upload local import backup to private GitHub repo
- Move platform install control to QuickSettings path row
- Remove main UI PlatformInstallButton
"""

FILES = [
    "crates/cockpit-core/src/modules/cursor_account.rs",
    "src-tauri/src/lib.rs",
    "src-tauri/src/modules/cursor_account.rs",
    "src-tauri/src/modules/platform_installer.rs",
    "src/App.tsx",
    "src/components/QuickSettingsPopover.tsx",
    "src/pages/SettingsPage.tsx",
]


def token() -> str:
    for key in ("GITHUB_TOKEN", "GH_TOKEN"):
        import os

        value = os.environ.get(key, "").strip()
        if value:
            return value
    try:
        out = subprocess.check_output(["gh", "auth", "token"], text=True, stderr=subprocess.DEVNULL)
        return out.strip()
    except Exception as exc:  # noqa: BLE001
        raise SystemExit(f"No GitHub token available: {exc}") from exc


def api(method: str, path: str, body: dict | None = None) -> dict:
    url = f"https://api.github.com{path}"
    data = None if body is None else json.dumps(body).encode("utf-8")
    req = urllib.request.Request(
        url,
        data=data,
        method=method,
        headers={
            "Authorization": f"Bearer {token()}",
            "Accept": "application/vnd.github+json",
            "User-Agent": "cockpit-tools-push-script",
            "Content-Type": "application/json",
        },
    )
    with urllib.request.urlopen(req) as resp:
        return json.load(resp)


def get_sha(path: str) -> str | None:
    try:
        payload = api("GET", f"/repos/{OWNER}/{REPO}/contents/{path}?ref={BRANCH}")
        return payload.get("sha")
    except urllib.error.HTTPError as err:
        if err.code == 404:
            return None
        raise


def main() -> int:
    updated: list[str] = []
    for rel in FILES:
        local = ROOT / rel
        if not local.is_file():
            print(f"SKIP missing: {rel}", file=sys.stderr)
            continue
        content_b64 = base64.b64encode(local.read_bytes()).decode("ascii")
        body: dict = {
            "message": MESSAGE if not updated else f"fork: push {rel}",
            "content": content_b64,
            "branch": BRANCH,
        }
        sha = get_sha(rel)
        if sha:
            body["sha"] = sha
        print(f"PUSH {rel} ({local.stat().st_size} bytes)", flush=True)
        api("PUT", f"/repos/{OWNER}/{REPO}/contents/{rel}", body)
        updated.append(rel)
    print("DONE", len(updated), "files")
    ref = api("GET", f"/repos/{OWNER}/{REPO}/git/ref/heads/{BRANCH}")
    print("HEAD", ref["object"]["sha"])
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
