# Cockpit Tools — Agent 项目上下文

> 本文件供 Codex/Cursor Agent 读取。配合 `.cursor/project-brief.md`（每会话更新）、`docs/FORK-DESIGN-ARCHIVE.md`（长期功能设计档案）与 lazy-mcp 项目上下文 MCP 使用。

## 项目是什么

**Cockpit Tools** 是通用 AI IDE 多账号管理桌面应用（Tauri 2 + React 19 + Rust workspace）。

本 fork 相对 upstream 的**验收基线与能用/不能用特征**以 [`.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc`](.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc) 为准。

- **本轮真合并交付候选**：安装 exe SHA `D6C85620…`（ProductVersion 1.3.0）；分支 `sync-upstream-v1.3.0-20260714-redo` @ `418b7bb2`；parents=`918980e9`+`da0deca4`；**用户确认前不标「已修好/能用」**
- **拼装拒收物**：SHA `1A0EC65E…` / tag `sync-upstream-v1.3.0-20260714` — **不能用**
- **合并前锚点**：`fork-20260705` @ `918980e9`；Draft `fork-20260705-baseline`
- **历史桌面验收包**：`Cockpit-nirvana-token-test.exe`（SHA `F5256511…`）
- **硬禁**：整棵换 `src/` 冒充同步；debug 当基线；第三套拼装 UI

## 仓库结构

| 路径 | 用途 |
|------|------|
| `src-tauri/` | Tauri GUI 主程序（`cockpit-tools` crate） |
| `crates/cockpit-core/` | 共享业务逻辑库 |
| `crates/cockpit-cli/` | CLI（`cockpit` 命令） |
| `scripts/` | 编译、部署、验证脚本 |
| `docs/CURSOR-FORK-SCOPE.md` | 你的版本 vs 开发仓 vs upstream 边界 |
| `docs/FORK-DESIGN-ARCHIVE.md` | fork 持久功能设计档案 |
| `.codex/rules/` | Codex 侧规则镜像 |
| `.cursor/rules/` | Agent 行为规则 |

## 数据与部署路径（Windows）

| 项 | 路径 |
|----|------|
| 开发仓 | `C:\Users\aliceemoce\dev\cockpit-tools` |
| Cursor 运行时账号 JSON | `%USERPROFILE%\.antigravity_cockpit\cursor_accounts\` |
| **禁止当 Cursor 池** | `%USERPROFILE%\.antigravity_cockpit\data\cursor_accounts\` |
| **本轮安装候选** | `%LocalAppData%\Cockpit Tools\cockpit-tools.exe`（SHA `D6C85620…`） |
| Release | `sync-upstream-v1.3.0-20260714-redo`（exe + NSIS setup） |
| 对照 PR | https://github.com/aliceemoce/cockpit-tools/pull/6 |
| 凭证镜像仓 | `C:\Users\aliceemoce\dev\cockpit-credentials` |

## 硬约束（Agent 必须遵守）

1. GUI 交付须 `npm run build` + `npm run tauri build`（release）
2. 先对照特征注册表；拼装货不得标能用
3. Cockpit UI 验收用 UIA MCP；画面非网络错误
4. GitHub 用已认证 `gh` / MCP
5. upstream 同步只响应正式 release/tag；须同时真用允许的 fork tip 与线上 tip
6. 先读并增量维护 `docs/FORK-DESIGN-ARCHIVE.md`
7. 历史要求见 `.cursor/user-history-requirements.md`
8. 对用户结论仅 **已修好** / **没修好**（用户确认前本轮不写已修好）
