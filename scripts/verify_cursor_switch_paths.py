#!/usr/bin/env python3
"""
静态断言：账号总览 Play 与多开 Start 必须走同一 Rust 入口。
退出码 0 = 通过；1 = 不一致或仍含旧 inject_bound 路径。
"""
from __future__ import annotations

import re
import sys
from pathlib import Path

REPO = Path(r"C:\Users\aliceemoce\dev\cockpit-tools")
CURSOR_RS = REPO / "src-tauri/src/commands/cursor.rs"
INSTANCE_RS = REPO / "src-tauri/src/commands/cursor_instance.rs"
ACCOUNT_RS = REPO / "src-tauri/src/modules/cursor_account.rs"


def read(p: Path) -> str:
    return p.read_text(encoding="utf-8")


def main() -> int:
    errors: list[str] = []
    cursor = read(CURSOR_RS)
    instance = read(INSTANCE_RS)
    account = read(ACCOUNT_RS)

    for label, text in [("cursor.rs", cursor), ("cursor_instance.rs", instance), ("cursor_account.rs", account)]:
        if "无忧" in text or "nirvana" in text.lower() or "jzzcg" in text.lower():
            errors.append(f"{label} 仍含外部小助手相关字符串（cockpit 不应引用）")

    if "inject_bound_account_for_instance_start" in instance:
        errors.append("cursor_instance.rs 仍含已废弃的 inject_bound_account_for_instance_start")

    if not re.search(
        r"start_cursor_instance_with_account_switch\s*\(\s*instance_id\s*,\s*None\s*\)",
        instance,
    ):
        errors.append("cursor_start_instance 未委托 start_cursor_instance_with_account_switch(instance_id, None)")

    if not re.search(
        r"start_cursor_instance_with_account_switch\s*\(\s*\"__default__\"",
        cursor,
    ):
        errors.append("inject_cursor_account 未委托 start_cursor_instance_with_account_switch(__default__, Some(...))")

    if "switch_cursor_account_to_profile" not in account:
        errors.append("cursor_account.rs 缺少 switch_cursor_account_to_profile")

    if "pub fn switch_cursor_account_to_profile" not in account:
        errors.append("switch_cursor_account_to_profile 未导出")

    # inject_to_cursor 仍可存在供本地导入，但 instance 启动链不得直接调用 inject_to_cursor 绕过 switch
    if re.search(r"inject_to_cursor\s*\(", instance):
        errors.append("cursor_instance 命令层仍直接调用 inject_to_cursor（应经 switch_cursor_account_to_profile）")

    if errors:
        print("FAIL:")
        for e in errors:
            print(f"  - {e}")
        return 1

    print("PASS: 总览 Play 与多开 Start 共用 start_cursor_instance_with_account_switch -> switch_cursor_account_to_profile")
    return 0


if __name__ == "__main__":
    sys.exit(main())
