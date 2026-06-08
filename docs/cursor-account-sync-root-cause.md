# Cursor 账号未同步 / 配额为零 — 根因与修复

## 现象

- 账号列表**不全**：索引里有条目，但部分账号从未出现在 UI。
- 部分账号**配额显示为零**或「暂无配额数据」：本地有账号记录，但 `cursor_usage_raw` 为空。
- 右上角 **「检测中 / 未检测到版本」** 程序状态徽章在 Cursor 等平台页缺失。

## 根因 1：备份恢复仅在「整表为空」时触发

`list_accounts()` / `list_accounts_checked()` 原先逻辑：

```text
normalize_account_index()
→ 仅当 accounts.is_empty() 时才 restore_missing_accounts_from_backups()
```

当索引中已有**部分**可读账号、另一些详情 JSON 丢失时，恢复逻辑**不会运行**，丢失的邮箱无法从 `cursor_local_import_backups/` 或私有仓库 `cursor-import-backups/` 补回。

`restore_missing_accounts_from_backups` 还只用**磁盘上已有**账号文件的邮箱做去重，索引里「仅有 summary、无详情文件」的邮箱会被误判为「未占用」，导致重复或漏合并。

## 修复（cockpit-tools）

1. **始终**在 `normalize_account_index` 之后调用 `restore_missing_accounts_from_backups`（不再要求 `accounts.is_empty()`）。
2. 新增 `collect_known_account_emails(index)`：合并「磁盘账号邮箱 + 索引 summary 邮箱」后再做备份去重。
3. `list_cursor_accounts` 命令启动时仍会 `pull_remote_import_backups()`；需配置 `COCKPIT_GITHUB_TOKEN` / `GITHUB_TOKEN`。
4. 新增 `restore_missing_detail_files_from_mirror()`：当索引有条目但 `~/.antigravity_cockpit/cursor_accounts/<id>.json` 缺失时，从 `COCKPIT_CREDENTIALS_DIR/data/cursor_accounts/`（或 `~/dev/cockpit-credentials/data/cursor_accounts/`）复制详情文件到本地数据目录。

## 根因 2：配额为零 vs 查询失败

| 情况 | 字段 | UI 表现 |
|------|------|---------|
| 从未刷新配额 | `cursor_usage_raw == null` | 0% / 无数据 |
| 刷新失败 | `quota_query_last_error` 有值 | 错误态；排序应**置后** |

导入（本机/OAuth/JSON）默认不拉配额，需手动或「刷新全部」后才有 `cursor_usage_raw`。

## 根因 3：右上角程序状态徽章被占位符替换

- Antigravity 总览页：`OverviewTabsHeader` → `AntigravityInstalledVersionBadge`（正常）。
- Cursor/Codex 等平台页：`PlatformOverviewTabsHeader` 使用 `page-top-strip-right-placeholder`，**不渲染**检测徽章 → UI 验收无法确认「程序已安装/路径有效」。

修复：新增 `PlatformInstalledVersionBadge`，在 `PlatformOverviewTabsHeader` 右侧调用 `detect_app_path` 显示安装状态（与主仓库 Antigravity 徽章同一视觉规范）。

## 排序

- Cursor 账号页默认排序：`defaultSortBy: 'credits'`（剩余 Credits 降序）。
- `quota_query_last_error` 非空的账号在任意排序键下**排在最后**（当前账号仍优先）。

## 验证清单

1. 打开 Cursor 账号页 → 右上角应出现「检测中」→「Cursor + 可执行文件名」或「未检测到版本」。
2. 删除单个 `cursor_accounts/<id>.json` 但保留索引 → 刷新列表应从 `cursor_local_import_backups/` 恢复（若备份存在）。
3. 配置 GitHub Token → `list_cursor_accounts` 应从 `aliceemoce/cockpit-credentials` 拉取缺失邮箱。
4. 刷新全部后，有 token 的账号应写入 `cursor_usage_raw`；失败账号带 `quota_query_last_error` 且排在列表末尾。
