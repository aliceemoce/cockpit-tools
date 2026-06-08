# Cursor 切号：主仓库 vs fork

## 1. cockpit-tools 内无「无忧小助手」相关代码

`jlcodes99/cockpit-tools` 与 `aliceemoce/cockpit-tools` 源码中 **不存在** 任何 `无忧` / `nirvana` / `jzzcg` 引用。  
桌面上的 `无忧小助手.lnk` 指向独立程序 `C:\Program Files\nirvana\nirvana.exe`，与 Cockpit 仓库无关。

Cockpit 只实现自己的 Cursor 切号：`inject_cursor_account` / `start_cursor_instance_with_account_switch`。

## 2. Cursor 切号在 Cockpit 里一直存在

| 层级 | 入口 |
|------|------|
| 前端 | `injectCursorAccount()` → `invoke('inject_cursor_account')` |
| 后端 | `commands/cursor.rs::inject_cursor_account` |
| UI | 账号总览行内 **Play** 按钮 |

主仓库 `jlcodes99/cockpit-tools` **main** 与 **v0.24.8** 均有此命令，并非 fork 独有。

## 3. 三版切号流程对比

### 主仓库 v0.24.8 / main

**账号总览 Play：**
```text
inject_to_cursor
→ set_current_account_id
→ update_default_settings(bind)
→ cursor_start_instance("__default__")  // 内部可能再次 inject（若已 bind）
```

**多开实例 Start：**
```text
close_cursor
→ 仅当 bind_account_id 存在：inject_account_to_profile（无指纹重置）
→ 启动 Cursor
```

**问题：** 总览与多开 **不一致**；总览可能 **二次注入**；均无指纹重置。

### aliceemoce/main（GitHub 已推送，2026-06-06 前后）

**账号总览：** 增加 `hard_reset` + `inject_to_cursor`，再 `cursor_start_instance`（仍可能二次注入）。

**多开：** `inject_bound` 含 `hard_reset`，但 **无 bind 则跳过注入**。

**问题：** 总览有多余的 close/reset/inject，启动时又走一遍 inject_bound — **仍不一致**。

### 本地 fork（本次修复，待 push）

**统一入口：** `start_cursor_instance_with_account_switch(instance_id, forced_account_id?)`

```text
resolve 账号（forced | bind | 当前账号）
→ switch_cursor_account_to_profile（close + hard_reset + inject）
→ set_current_account_id
→ 总览 Play 时更新 default bind
→ cursor_start_instance_prepared（仅 close 残留进程 + 启动，不二次注入）
```

- **账号总览 Play：** `start_cursor_instance_with_account_switch("__default__", Some(account_id))`
- **多开 Start：** `start_cursor_instance_with_account_switch(instance_id, None)`

## 4. 右上角徽章 / 静默安装

| 版本 | Platform 页右上角 |
|------|-------------------|
| 主仓库 main | `PlatformOverviewTabsHeader` 右侧为 **placeholder**，无检测徽章 |
| fork | `PlatformInstalledVersionBadge` + 未检测时 **静默安装** |

主仓库 **无** `platform_installer.rs`（v0.24.8）；fork 从后续版本合并并接徽章。

## 5. 为何你这边「全错」

1. 桌面 Cockpit 快捷方式 → `%LocalAppData%\Cockpit Tools\` **旧 exe**（6/6），不是 dev release（6/8）。
2. GitHub `aliceemoce/main` 仍是 **不一致** 的切号逻辑（见上表）。
3. Agent 未做 **静默安装端到端** 实测（需管理员 + 清 `cursor_app_path`）。

## 6. 验证步骤（必须本地执行）

```powershell
cd C:\Users\aliceemoce\dev\cockpit-tools
npm run build
$env:CARGO_TARGET_DIR="C:\Users\aliceemoce\dev\cargo-target\cockpit-tools"
cargo build --release
Copy-Item "C:\Users\aliceemoce\dev\cargo-target\cockpit-tools\release\cockpit-tools.exe" `
  "$env:LOCALAPPDATA\Cockpit Tools\cockpit-tools.exe" -Force
python scripts\verify_acceptance_minimized.py
```

切号自测：总览 Play 与多开（同 bind 账号）应对同一 profile 写入相同 `cursorAuth/accessToken`。
