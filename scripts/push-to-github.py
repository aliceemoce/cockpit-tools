#!/usr/bin/env python3
"""Push staged fork changes to aliceemoce/cockpit-tools via git."""
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parents[1]
VERIFY_SRC = Path(
    r"C:\Users\aliceemoce\.cursor\projects\e-Users-alice-cursor-projects-empty-window-cockpit-tools-alice\COCKPIT_UI_VERIFY.md"
)

FILES = [
    "COCKPIT_UI_VERIFY.md",
    "crates/cockpit-core/src/modules/cursor_account.rs",
    "src-tauri/src/commands/mod.rs",
    "src-tauri/src/lib.rs",
    "src-tauri/src/modules/cursor_account.rs",
    "src-tauri/src/modules/mod.rs",
    "src-tauri/src/modules/platform_installer.rs",
    "src-tauri/src/modules/cursor_import_backup_sync.rs",
    "src/App.tsx",
    "src/components/PlatformInstallButton.tsx",
    "src/components/QuickSettingsPopover.css",
    "src/components/QuickSettingsPopover.tsx",
    "src/pages/SettingsPage.tsx",
]

MSG = """fork: cursor refresh email, import backup sync, quick-settings install path

- Write API email to account.email on cursor refresh
- Upload local import backup to private GitHub repo
- Move platform install control to QuickSettings path row
- Remove main UI PlatformInstallButton
- Add COCKPIT_UI_VERIFY.md for non-foreground UIA verification
"""


def run(cmd: list[str]) -> None:
    print("+", " ".join(cmd), flush=True)
    subprocess.run(cmd, cwd=ROOT, check=True)


def main() -> int:
    if VERIFY_SRC.is_file():
        (ROOT / "COCKPIT_UI_VERIFY.md").write_text(
            VERIFY_SRC.read_text(encoding="utf-8"), encoding="utf-8"
        )
    run(["git", "add", *FILES])
    run(["git", "diff", "--cached", "--stat"])
    run(["git", "commit", "-m", MSG])
    run(["git", "push", "origin", "main"])
    run(["git", "log", "-1", "--oneline"])
    run(["git", "status", "--short"])
    return 0


if __name__ == "__main__":
    sys.exit(main())
