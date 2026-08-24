
| `cursor_account` | 账号落盘、usage-summary 刷新、切号 pick；**默认/多开共用** `switch_cursor_account_to_profile` 无感写库 |
| `cursor_instance` | 多开 profile、启动路径（默认 Program Files / 多开 Cursor-Multi）；默认已在跑则跳过二次 launch |
| `cursor_chat_probe` | 隔离 APPDATA + Agent CLI 最小对话验活（**辅助**，不代替 IDE 验收） |
| `CursorAccountsPage` | 展示 chat_probe 与 usage 百分比（口径分离）；左 Play=默认 profile，右无感=多开首实例 |
1. **默认 Play（左边）**：`inject_cursor_account` → `__default__` → `%APPDATA%\Cursor` + 通常 `%ProgramFiles%\Cursor\Cursor.exe`；无感写库、禁止杀默认窗
2. **多开 Play（应用多开行 / 右无感）**：`cursor_start_instance` → 实例 profile + `C:\Cursor-Multi\Cursor.exe`；禁止杀窗
3. 账号 JWT → 隔离 `auth.json` → `agent -p` → `chat_probe`（CLI 验活，与 IDE 验活分开记）
4. 前端仅当 `chat_probe.outcome == ok` 显示「可对话」；**用户验收标准**仍以 IDE 内对话为准
- 双安装：默认 `%ProgramFiles%\Cursor`；多开 `C:\Cursor-Multi`（未注入续杯管家）