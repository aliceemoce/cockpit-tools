# Cockpit Tools — Agent 项目上下文

> 本文件供 Codex/Cursor Agent 读取。配合 `.cursor/project-brief.md`（每会话更新）、`docs/FORK-DESIGN-ARCHIVE.md`（长期功能设计档案）与 lazy-mcp 项目上下文 MCP 使用。

## 项目是什么

**Cockpit Tools** 是通用 AI IDE 多账号管理桌面应用（Tauri 2 + React 19 + Rust workspace）。

本 fork 相对 upstream 的**验收基线与能用/不能用特征**以 [`.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc`](.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc) 为准（用户纠正后增量更新；优先于本文旧描述）。

- **当前最新构建**：`%LocalAppData%\Cockpit Tools\cockpit-tools.exe`（SHA `1A0EC65E…`，ProductVersion **1.3.0**，**release**，2026-07-14）
- **源码交付分支**：`sync-upstream-v1.3.0-20260714-full` @ `102904ee`（对照 [PR #5](https://github.com/aliceemoce/cockpit-tools/pull/5)）
- **上游锚点**：`jlcodes99/cockpit-tools` tag **`v1.3.0`** / `da0deca4`
- **历史桌面验收包**：`%USERPROFILE%\Desktop\Cockpit-nirvana-token-test.exe`（SHA `F5256511…`）
- **release 范围边界**：[`docs/CURSOR-FORK-SCOPE.md`](docs/CURSOR-FORK-SCOPE.md)（若与注册表冲突，先改注册表再同步文档）
- **硬禁**：把 **debug** 当最新基线；为过编译删减上游平台 UI 却宣称「已同步 v1.x」

## 仓库结构

| 路径 | 用途 |
|------|------|
| `src-tauri/` | Tauri GUI 主程序（`cockpit-tools` crate） |
| `crates/cockpit-core/` | 共享业务逻辑库 |
| `crates/cockpit-cli/` | CLI（`cockpit` 命令） |
| `scripts/` | 编译、部署、验证脚本 |
| `docs/CURSOR-FORK-SCOPE.md` | 你的版本 vs 开发仓 vs upstream 边界 |
| `docs/FORK-DESIGN-ARCHIVE.md` | fork 持久功能设计档案 |
| `.codex/rules/` | Codex 侧规则镜像；须通过本文和自动化 prompt 显式读取 |
| `.cursor/rules/` | Agent 行为规则 |

## 数据与部署路径（Windows）

| 项 | 路径 |
|----|------|
| 开发仓 | `C:\Users\aliceemoce\dev\cockpit-tools` |
| 运行时账号数据 | `%USERPROFILE%\.antigravity_cockpit` |
| **Cursor 运行时账号 JSON** | `%USERPROFILE%\.antigravity_cockpit\cursor_accounts\` |
| **禁止当 Cursor 池** | `%USERPROFILE%\.antigravity_cockpit\data\cursor_accounts\` |
| **当前最新安装/运行 exe** | `%LocalAppData%\Cockpit Tools\cockpit-tools.exe`（SHA `1A0EC65E…`） |
| Release tag | `sync-upstream-v1.3.0-20260714` → `cockpit-tools.exe` |
| 凭证镜像仓 | `C:\Users\aliceemoce\dev\cockpit-credentials` |

## 硬约束（Agent 必须遵守）

1. **本机编译** — GUI 交付须 `npm run build` + `npm run tauri build`（release）；禁止仅 `cargo build` 覆盖安装
2. **先对照特征注册表**区分当前最新构建、历史桌面验收包、debug 拒收物
3. **Cockpit UI 验收** 用 UIA MCP；画面须非「网络错误」
4. **GitHub API** 用 `user-github` MCP 或 `gh`，禁止未认证 fetch
5. **upstream 同步**只响应正式 release/tag（F-007）；须完整合入对应版本前端/接线（F-008），保留 Cursor fork 边界
6. **先看后改、改后维护** — 先读 `docs/FORK-DESIGN-ARCHIVE.md`；实际修改后增量维护
7. **历史要求** — `.cursor/user-history-requirements.md`；冲突时询问用户裁决
8. **修好结论** — 只报 **已修好** 或 **没修好**（`honest-fix-status-only.mdc`）

## 会话启动协议（阻塞）

每个 Agent 会话在改代码或跑 shell **之前**必须：

1. 阅读本文件、`docs/CURSOR-FORK-SCOPE.md`、`docs/FORK-DESIGN-ARCHIVE.md`、设计档案维护规则、`.cursor/user-history-requirements.md` 与 `.cursor/project-brief.md`
2. 按需调用项目上下文 MCP
3. 更新 `.cursor/project-brief.md` 并创建 `.cursor/.brief-ready`
4. 在用户可见回复开头输出 **📋 项目理解** 摘要
