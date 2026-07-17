# 工作区设计

## 子系统

| 模块 | 职责 |
|------|------|
| `cursor_account` | 账号落盘、usage-summary 刷新、切号 pick |
| `cursor_chat_probe` | 隔离 APPDATA + Agent CLI 最小对话验活 |
| `CursorAccountsPage` | 展示 chat_probe 与 usage 百分比（口径分离） |

## 主数据流

1. 账号 JWT → 隔离 `auth.json` → `agent -p --mode ask` → 分类 outcome → 写入 `chat_probe`
2. 前端仅当 `chat_probe.outcome == ok` 显示「可对话」
3. Play 选号优先 `chat_ok` 池；`rate_limited` / `auth_failed` 排除

## 外部依赖

- 本机 Cursor Agent CLI（`%LOCALAPPDATA%\cursor-agent\agent.ps1`）
- `https://cursor.com/api/usage-summary`（仅消费统计，不代表可对话）
