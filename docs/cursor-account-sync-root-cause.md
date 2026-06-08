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

当索引中已有**部分**可读账号、另一些详情 JSON 丢失时，恢复逻辑**不会运行**。

`restore_missing_accounts_from_backups` 还只用**磁盘上已有**账号文件的邮箱做去重，索引里「仅有 summary、无详情文件」的邮箱会被误判。

## 修复

1. **始终**在 normalize 之后调用 `restore_missing_accounts_from_backups`。
2. 新增 `collect_known_account_emails(index)` 合并索引 summary 邮箱。
3. `list_cursor_accounts` 仍会 `pull_remote_import_backups()`（需 GitHub Token）。

## 根因 2：配额为零

导入默认不拉配额；需刷新。`quota_query_last_error` 表示查询失败，排序置后。

## 根因 3：右上角程序状态徽章

`PlatformOverviewTabsHeader` 使用 `page-top-strip-right-placeholder` 占位，未渲染 `PlatformInstalledVersionBadge`。

## 排序

- 默认 `credits`；配额查询失败排最后。
