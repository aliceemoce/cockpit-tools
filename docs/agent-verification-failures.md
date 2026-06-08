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
