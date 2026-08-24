# Changelog

## 1.3.21 — 2026-08-22

- 默认无感写库对齐虚备：Cursor 占用库时不切 journal、不删 -wal，busy 重试
- 深链 `cockpit-tools://click/<action_id>` 触发应用内点击（验收 Zap）

## 1.3.20 — 2026-08-16

- 默认切号改为无感写库（不关正在用的默认窗）；无忧传统关窗路径保留
- 默认自动换号（总览 Zap / `inject_cursor_account_auto`，与多开「启动」同构自动选号）
- 写库后粘号复查（`stick_check`）；失败写审计与告警
- 额度显示：假 0% 标「核实中」；API 新 0% 不覆盖磁盘历史非 0
