#!/usr/bin/env python3
"""Resolve merge conflicts for upstream v1.3.16 sync → deliver 1.3.18."""
from __future__ import annotations

import re
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
NEW_VER = "1.3.18"


def resolve_simple_version(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    pat = re.compile(
        r"<<<<<<< HEAD\n(?P<head>.*?)\n=======\n(?P<theirs>.*?)\n>>>>>>> [^\n]+\n",
        re.S,
    )

    def repl(m: re.Match[str]) -> str:
        head, theirs = m.group("head"), m.group("theirs")
        blob = head + "\n" + theirs
        if '"version"' in blob:
            return f'  "version": "{NEW_VER}",\n'
        if re.search(r"^version\s*=", head, re.M) or re.search(
            r"^version\s*=", theirs, re.M
        ):
            return f'version = "{NEW_VER}"\n'
        raise SystemExit(f"Unhandled conflict in {path}:\n{m.group(0)}")

    new, n = pat.subn(repl, text)
    if "<<<<<<<" in new:
        raise SystemExit(f"Remaining conflicts in {path}")
    path.write_text(new, encoding="utf-8", newline="\n")
    print(f"{path.relative_to(ROOT)}: resolved {n} block(s) -> {NEW_VER}")


def take_both_changelogs(path: Path, is_zh: bool) -> None:
    text = path.read_text(encoding="utf-8")
    if "<<<<<<<" not in text:
        print(f"{path.name}: no conflicts")
        return

    # Strategy: prefer HEAD structure, inject upstream 1.3.16 section if missing,
    # then ensure 1.3.18 sync header at top of version list.
    # Simpler approach: for each conflict, if one side is version bump header only,
    # keep both unique sections ordered newest-first.

    pat = re.compile(
        r"<<<<<<< HEAD\n(?P<head>.*?)=======\n(?P<theirs>.*?)>>>>>>> [^\n]+\n",
        re.S,
    )

    def merge_block(m: re.Match[str]) -> str:
        head = m.group("head")
        theirs = m.group("theirs")
        # Version-only line conflicts already handled elsewhere; here keep both
        # with HEAD first then theirs if not subset.
        if head.strip() == theirs.strip():
            return head
        # Prefer concatenating: theirs (upstream newer section) then head extras
        # but avoid duplicating identical ## headers.
        return theirs + head

    new = pat.sub(merge_block, text)
    if "<<<<<<<" in new:
        raise SystemExit(f"Remaining conflicts in {path}")

    # Prepend 1.3.18 delivery note after the main title / before first ## [1.
    note_en = (
        f"## [{NEW_VER}] - 2026-08-04\n\n"
        "### Changed\n\n"
        "- **Fork sync**: merged upstream formal release **v1.3.16** into fork tip 1.3.17; "
        "kept Cursor local watch (`sync_cursor_local_watch` / `accounts:changed`), "
        "rotation pick, email-dedup release boundary, and default-instance non-global kill.\n\n"
    )
    note_zh = (
        f"## [{NEW_VER}] - 2026-08-04\n\n"
        "### 变更\n\n"
        "- **Fork 同步**：将主仓正式版 **v1.3.16** 合入 fork tip 1.3.17；"
        "保留 Cursor 本地跟号（`sync_cursor_local_watch` / `accounts:changed`）、"
        "强制轮换选号、邮箱去重 release 边界、默认实例不全杀。\n\n"
    )
    note = note_zh if is_zh else note_en

    # Insert after first occurrence of a version header area
    m = re.search(r"(^## \[1\.)", new, re.M)
    if m and f"## [{NEW_VER}]" not in new:
        new = new[: m.start()] + note + new[m.start() :]

    # Normalize any leftover 1.3.17/1.3.16 product version claims in badges if present
    path.write_text(new, encoding="utf-8", newline="\n")
    print(f"{path.relative_to(ROOT)}: changelog merged + {NEW_VER} note")


def resolve_readme(path: Path) -> None:
    text = path.read_text(encoding="utf-8")
    pat = re.compile(
        r"<<<<<<< HEAD\n(?P<head>.*?)=======\n(?P<theirs>.*?)>>>>>>> [^\n]+\n",
        re.S,
    )

    def repl(m: re.Match[str]) -> str:
        head, theirs = m.group("head"), m.group("theirs")
        # Prefer upstream README for product docs, but bump version strings to NEW_VER
        chosen = theirs if len(theirs) >= len(head) else head
        # If both are short version mentions, take theirs then patch
        if "1.3.1" in head or "1.3.1" in theirs:
            chosen = theirs if "1.3.16" in theirs else head
        chosen = re.sub(r"1\.3\.(16|17)", NEW_VER, chosen)
        return chosen

    new = pat.sub(repl, text)
    if "<<<<<<<" in new:
        raise SystemExit(f"Remaining conflicts in {path}")
    new = re.sub(r"\b1\.3\.(16|17)\b", NEW_VER, new)
    path.write_text(new, encoding="utf-8", newline="\n")
    print(f"{path.relative_to(ROOT)}: readme resolved")


def main() -> None:
    for rel in [
        "package.json",
        "src-tauri/Cargo.toml",
        "src-tauri/tauri.conf.json",
    ]:
        resolve_simple_version(ROOT / rel)

    take_both_changelogs(ROOT / "CHANGELOG.md", is_zh=False)
    take_both_changelogs(ROOT / "CHANGELOG.zh-CN.md", is_zh=True)
    resolve_readme(ROOT / "README.md")
    resolve_readme(ROOT / "README.en.md")

    # Cargo.lock: take HEAD then regenerate via cargo later; for now checkout --ours
    lock = ROOT / "Cargo.lock"
    if "<<<<<<<" in lock.read_text(encoding="utf-8", errors="replace"):
        print("Cargo.lock: will checkout --ours then cargo update versions")


if __name__ == "__main__":
    main()
