# Agent 自验失败点复盘（为何用户端「全错」）

## 1. 测了错误的二进制

| 失败 | 后果 |
|------|------|
| 只跑 `cargo build --release`，未先 `npm run build` | 前端仍是旧 dist，徽章/排序改动不可见 |
| 未启用 `custom-protocol`（Tauri release） | WebView 连 `localhost:1420`，页面 `ERR_CONNECTION_REFUSED` |
| 测 `cockpit-tools/release/` 复制物或旧安装包 | 与 `cargo-target` 最新 exe 不一致 |

**正确顺序：** `npm run build` → `cargo build --release`（`Cargo.toml` 含 `custom-protocol`）→ 启动 `cargo-target/.../release/cockpit-tools.exe`。

## 2. 测了错误的数据目录

账号在 **`%USERPROFILE%\.antigravity_cockpit`**（`cursor_accounts.json` + `cursor_accounts/`），不是 `%LocalAppData%\cockpit-tools`。

## 3. 右上角徽章：代码有、但 Agent 未验到

- **根因：** `PlatformOverviewTabsHeader` 长期用 `page-top-strip-right-placeholder`，平台页不渲染 `AntigravityInstalledVersionBadge`。
- **修复：** `PlatformInstalledVersionBadge` + `detect_app_path`。
- **Agent 漏检：** 未在 Cursor 平台页截图；仪表盘右上角本来就没有该徽章（只有 Antigravity 总览/平台 Tab 页才有）。

## 4. 「账号为零」≠「未同步」

审计（2026-06-08）：索引 **1242** = 详情文件 **1242**，`missing_detail_count: 0`。

| 现象 | 原因 |
|------|------|
| 配额 0% / 无数据 | `cursor_usage_raw` 为空（导入后未刷新配额），`null_usage_files: 19` |
| 查询失败置后 | `quota_query_last_error` 有值，`quota_error_files: 695` |

## 5. UIA 局限

Tauri WebView 内 DOM（如「检测中」徽章）多数不在 UIA 命名树；须 **截图** 或读 **dist bundle**（`installed-version-badge`）验证，不能仅凭 `find_elements` 空结果判定失败。

## 6. 深链导航误开导入框

`cockpit-tools://import?provider=cursor&token=...` 会打开「添加 Cursor 账号」模态，遮挡右上角徽章。应用 **侧栏 Invoke「Cursor」** 导航。

## 7. 干扰用户桌面

验收须 **Invoke 最小化**（`scripts/launch_and_minimize_cockpit.py` / `verify_acceptance_minimized.py`），禁止 `perform_action(click)` 抢焦点。

## 8. 只审计 Cursor、忽略全平台总量

仪表盘 **账号总数** = 各平台索引之和（含 Windsurf ~1300+、Cursor ~1242 等），不是单看 Cursor。

「超过 1242」常见原因：按 **详情 JSON 文件数**（含邮箱去重前的孤儿文件）计数，或把多平台加总。

## 9. 徽章只做 detect、未接静默安装

Quick Settings 路径行有 `install_missing_platform`（`/S` 等静默参数），早期 `PlatformInstalledVersionBadge` 仅 `detect_app_path`。未检测到时应显示 **静默安装** 按钮（与 QS 同一后端）。

## 10. Cursor 多开实例 vs 账号总览 Play

| | 账号总览 Play (`inject_cursor_account`) | 多开实例 Start |
|--|----------------------------------------|----------------|
| 指纹重置 | 默认 profile `hard_reset` | 原先**无**（已补 `hard_reset_cursor_fingerprint_state_for_profile`） |
| 注入 | 总是注入所选账号 | 仅当实例 **绑定账号** 时注入 |
| 跟随当前账号 | N/A | Cursor **不支持** `follow_local_account` |

未绑定账号的实例会打开空 profile，表现为「无法登录」——需在实例上绑定账号后再启动。

## 11. 换号逻辑 vs「无忧小助手」

本仓库 **无**「无忧小助手」字符串或专用协议。Cursor 切号为 Cockpit 自研路径：

`close_cursor` → `hard_reset_cursor_fingerprint_state` → `inject_to_cursor` → `cursor_start_instance`

日志中的「n 风格指纹重置」指本地 machineId/storage.json/state.vscdb 重写，**不是**第三方无忧小助手逻辑。

Antigravity 才有 WebSocket `seamless` 切号；Cursor **没有** seamless 插件切号。
