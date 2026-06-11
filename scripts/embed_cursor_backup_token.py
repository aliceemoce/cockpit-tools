#!/usr/bin/env python3
"""生成嵌入用加密 GitHub Token 二进制（不写入明文到仓库）。"""
from __future__ import annotations

import hashlib
import os
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
OUT = ROOT / "src-tauri" / "embedded" / "cursor_backup_token.bin"

KEY_PARTS = (
    b"cockpit-tools-cursor-backup-v1",
    b"aliceemoce/cockpit-credentials",
    b"cursor-import-backups",
)


def derive_key() -> bytes:
    h = hashlib.sha256()
    for part in KEY_PARTS:
        h.update(part)
    return h.digest()


def read_token() -> str:
    for key in ("COCKPIT_GITHUB_TOKEN", "GITHUB_TOKEN", "GH_TOKEN"):
        value = os.environ.get(key, "").strip()
        if value:
            return value
    try:
        proc = subprocess.run(
            ["gh", "auth", "token"],
            capture_output=True,
            text=True,
            timeout=15,
            check=True,
        )
        token = proc.stdout.strip()
        if token:
            return token
    except (subprocess.SubprocessError, FileNotFoundError):
        pass
    raise SystemExit(
        "未找到 Token：请设置 COCKPIT_GITHUB_TOKEN / GITHUB_TOKEN / GH_TOKEN，或先 gh auth login"
    )


def main() -> int:
    try:
        from cryptography.hazmat.primitives.ciphers.aead import AESGCM
    except ImportError:
        print("需要 cryptography 包: pip install cryptography", file=sys.stderr)
        return 1

    token = read_token()
    key = derive_key()
    nonce = os.urandom(12)
    aes = AESGCM(key)
    ciphertext = aes.encrypt(nonce, token.encode("utf-8"), None)
    OUT.parent.mkdir(parents=True, exist_ok=True)
    OUT.write_bytes(nonce + ciphertext)
    print(f"已写入加密载荷: {OUT} ({OUT.stat().st_size} bytes)")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
