# Cockpit Tools — 会话简报

**日期**: 2026-06-22  
**任务**: Windsurf 账号「一个也打不开」排查  
**验收 exe**: `Desktop\Cockpit-nirvana-token-test.exe` SHA `F5256511…`（Cursor 链，与 Windsurf 无关）

## 根因（已核实）

- 索引 1336 个 Windsurf 账号，外层 `~/.antigravity_cockpit/windsurf_accounts/*.json` **全部 token 为空**，`quota_query_last_error` =「Token 不能为空」
- 完整凭证仍在嵌套目录 `windsurf_accounts/windsurf_accounts/*.json`（同 ID 1336 个可恢复）
- 切号时 `inject_account_to_profile` 因无 token 失败 → Windsurf 无法注入/启动登录态

## 本次动作

- 从嵌套目录合并恢复外层账号文件的 token 与 auth 字段
- 空壳文件备份至 `windsurf_accounts_empty_shell_backup_*`

## Cursor 基线（不变）

- 分支 `agent-build-20260615` @ `44321ebf`；默认切号不全杀 Cursor；transient 不写盘
