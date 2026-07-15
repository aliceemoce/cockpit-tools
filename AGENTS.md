# Cockpit Tools �?Agent 项目上下�?

> 本文件供 Codex/Cursor Agent 读取。配�?`.cursor/project-brief.md`（每会话更新）、`docs/FORK-DESIGN-ARCHIVE.md`（长期功能设计档案）�?lazy-mcp 项目上下�?MCP 使用�?

## 项目是什�?

**Cockpit Tools** 是通用 AI IDE 多账号管理桌面应用（Tauri 2 + React 19 + Rust workspace）�?

�?fork 相对 upstream �?*验收基线与能�?不能用特�?*�?[`.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc`](.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc) 为准�?

- **当前最新构建（能用�?*：安�?exe SHA `22778930…`（ProductVersion **1.3.4**）；分支 `sync-upstream-v1.3.4-20260715` @ `51fc9451`；parents=`dd920a4f`+`2d8f0fc2`（upstream **v1.3.4**）；含常�?`sync_cursor_local_watch`
- **上一可用 1.3.2**：SHA `3374C285…`；`sync-upstream-v1.3.2-20260715`；PR #7
- **上一可用 1.3.0**：SHA `DEA43F62…`；`sync-upstream-v1.3.0-20260714-redo`
- **拼装拒收�?*：SHA `1A0EC65E…` �?**不能�?*
- **合并前锚�?*：`fork-20260705` @ `918980e9`；Draft `fork-20260705-baseline`
- **历史桌面验收�?*：`Cockpit-nirvana-token-test.exe`（SHA `F5256511…`�?
- **硬禁**：整棵换 `src/` 冒充同步；debug 当基线；第三套拼�?UI；计划勾选冒充用户要求完�?

## 仓库结构

| 路径 | 用�?|
|------|------|
| `src-tauri/` | Tauri GUI 主程序（`cockpit-tools` crate�?|
| `crates/cockpit-core/` | 共享业务逻辑�?|
| `crates/cockpit-cli/` | CLI（`cockpit` 命令�?|
| `scripts/` | 编译、部署、验证脚�?|
| `docs/CURSOR-FORK-SCOPE.md` | 你的版本 vs 开发仓 vs upstream 边界 |
| `docs/FORK-DESIGN-ARCHIVE.md` | fork 持久功能设计档案 |
| `.codex/rules/` | Codex 侧规则镜�?|
| `.cursor/rules/` | Agent 行为规则 |

## 数据与部署路径（Windows�?

| �?| 路径 |
|----|------|
| 开发仓 | `C:\Users\aliceemoce\dev\cockpit-tools` |
| Cursor 运行时账�?JSON | `%USERPROFILE%\.antigravity_cockpit\cursor_accounts\` |
| **禁止�?Cursor �?* | `%USERPROFILE%\.antigravity_cockpit\data\cursor_accounts\` |
| **本轮安装** | `%LocalAppData%\Cockpit Tools\cockpit-tools.exe`（SHA `22778930…`，v1.3.4�?|
| Release | `sync-upstream-v1.3.4-20260715`（exe�?|
| 对照 PR | https://github.com/aliceemoce/cockpit-tools/pull/8 |
| 凭证镜像�?| `C:\Users\aliceemoce\dev\cockpit-credentials` |

## 硬约束（Agent 必须遵守�?

1. GUI 交付�?`npm run build` + `npm run tauri build`（release�?
2. 先对照特征注册表；拼装货不得标能�?
3. Cockpit UI 验收�?UIA MCP；画面非网络错误
4. GitHub 用已认证 `gh` / MCP
5. upstream 同步只响应正�?release/tag；须同时真用允许�?fork tip 与线�?tip
6. 先读并增量维�?`docs/FORK-DESIGN-ARCHIVE.md`
7. 历史要求�?`.cursor/user-history-requirements.md`
8. 对用户结论仅 **已修�?* / **没修�?*（验收由 Agent 用磁�?UIA 证据自决；禁止把「等用户确认」当完成门槛�?
9. **用户要求 > 计划勾�?* �?�?`.cursor/rules/user-requirements-over-plan-ceremony.mdc`；禁止用 merge/SHA/截图/PR 冒充完成；fork 行为须在可运行产品里
