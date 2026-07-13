# Cockpit Tools — Agent 项目上下文

> 本文件供 Codex/Cursor Agent 读取。配合 `.cursor/project-brief.md`（每会话更新）、`docs/FORK-DESIGN-ARCHIVE.md`（长期功能设计档案）与 lazy-mcp 项目上下文 MCP 使用。

## 项目是什么

**Cockpit Tools** 是通用 AI IDE 多账号管理桌面应用（Tauri 2 + React 19 + Rust workspace）。

本 fork 相对 upstream 的**验收基线与能用/不能用特征**以 [`.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc`](.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc) 为准（用户纠正后增量更新；优先于本文旧描述）。

- **当前最新构建**：`%LocalAppData%\Cockpit Tools\cockpit-tools.exe`（SHA `09BA38D3…`，v1.3.0，`sync-upstream-v1.3.0-20260714` @ `b0582bab`，2026-07-14）
- **历史桌面验收包**：`%USERPROFILE%\Desktop\Cockpit-nirvana-token-test.exe`（SHA `F5256511…`，分支 `agent-build-20260615` @ `44321ebf`）
- **release 范围边界**：[`docs/CURSOR-FORK-SCOPE.md`](docs/CURSOR-FORK-SCOPE.md)（若与注册表冲突，先改注册表再同步文档）
- **不在 release 内**：额度池合并、刷新 scheduler/batch 等（勿擅自扩展；邮箱去重已裁决保留）

## 仓库结构

| 路径 | 用途 |
|------|------|
| `src-tauri/` | Tauri GUI 主程序（`cockpit-tools` crate） |
| `crates/cockpit-core/` | 共享业务逻辑库 |
| `crates/cockpit-cli/` | CLI（`cockpit` 命令） |
| `scripts/` | 编译、部署、验证脚本 |
| `docs/CURSOR-FORK-SCOPE.md` | 你的版本 vs 开发仓 vs upstream 边界 |
| `docs/FORK-DESIGN-ARCHIVE.md` | fork 持久功能设计档案：每个功能的用户目的、自然语言设计、代码设计、保留边界、验证方式、增量记录 |
| `.codex/rules/` | Codex 侧规则镜像；必须通过本文和自动化 prompt 显式读取，不假设自动生效 |
| `.cursor/rules/` | Agent 行为规则 |

## 数据与部署路径（Windows）

| 项 | 路径 |
|----|------|
| 开发仓 | `C:\Users\aliceemoce\dev\cockpit-tools` |
| 运行时账号数据 | `%USERPROFILE%\.antigravity_cockpit` |
| **Cursor 运行时账号 JSON** | `%USERPROFILE%\.antigravity_cockpit\cursor_accounts\`（索引：`cursor_accounts.json`） |
| **禁止当 Cursor 池** | `%USERPROFILE%\.antigravity_cockpit\data\cursor_accounts\`（见 `.cursor/rules/cockpit-cursor-data-paths.mdc`） |
| 桌面 Cursor 运行监控 | `%USERPROFILE%\Desktop\CursorRunningAccounts\` |
| **当前最新安装/运行 exe** | `%LocalAppData%\Cockpit Tools\cockpit-tools.exe`（SHA `09BA38D3…`，v1.3.0，见特征注册表） |
| **历史桌面验收包** | `%USERPROFILE%\Desktop\Cockpit-nirvana-token-test.exe`（SHA `F5256511…`，见特征注册表） |
| 历史备份 | `Desktop\Cockpit-nirvana-token-20260614\`（`9791D94F`） |
| 误标/拒收 | `CB525188` staging r2、`7761f453`、`94B7D986` 等 Agent 中间包 |
| 凭证镜像仓 | `C:\Users\aliceemoce\dev\cockpit-credentials` |

## 硬约束（Agent 必须遵守）

1. **本机编译** — 直接 `cargo build` 或 `npm run tauri build`；完整 GUI 须含前端构建
2. **先对照特征注册表区分「当前最新构建」与「历史桌面验收包」**；用户说“最新构建/当前运行”默认指 `09BA38D3…`（v1.3.0），未经说明不要回退成 `F5256511…`
3. **Cockpit UI 验收** 用 `uia-pattern` 的 `ui_invoke`（无鼠标、无抢前台）
4. **GitHub API** 用 `user-github` MCP 或 `gh`，禁止未认证 fetch
5. **勿擅自加** 邮箱去重、额度池合并、刷新 scheduler/batch、配额 Agent 方案，除非用户明确开口
6. **先看后改、改后维护** — 任何手动 fork、自动化任务、代码或文档更新前，都要先查看 `docs/FORK-DESIGN-ARCHIVE.md`；只要本次实际修改了内容，结束后必须按功能增量维护该档案
7. **Codex 规则入口** — `.codex/rules/fork-design-archive-maintenance.md` 是 Codex 侧规则镜像；`.cursor/rules/` 是 Cursor 侧规则镜像。Codex 不能假设任何镜像目录自动生效，必须通过本文件、自动化 prompt 和实际读取来执行同一约束
8. **历史要求** — `.cursor/user-history-requirements.md` 累积用户要求；任务不得违背；冲突时询问用户裁决（见 `.cursor/rules/user-history-requirements.mdc`）
9. **修好结论** — 对用户只报 **已修好** 或 **没修好**（见 `.cursor/rules/honest-fix-status-only.mdc`）；禁止「仅后端」「仅编译」等分拆话术冒充修好

## 会话启动协议（阻塞）

每个 Agent 会话在改代码或跑 shell **之前**必须：

1. 阅读本文件、[`docs/CURSOR-FORK-SCOPE.md`](docs/CURSOR-FORK-SCOPE.md)、[`docs/FORK-DESIGN-ARCHIVE.md`](docs/FORK-DESIGN-ARCHIVE.md)、`.codex/rules/fork-design-archive-maintenance.md`、`.cursor/user-history-requirements.md` 与 `.cursor/project-brief.md`
2. 通过 **lazy-mcp** 按需调用至少一个项目上下文 MCP（见 `.cursor/rules/project-context-mcp.mdc`）
3. 用 200–500 字更新 `.cursor/project-brief.md`
4. 创建 `.cursor/.brief-ready`（内容：`ready`）
5. 在用户可见回复开头输出 **📋 项目理解** 摘要

## 主要平台能力

支持 Antigravity、Codex、Copilot、Windsurf、Kiro、**Cursor**、Gemini CLI 等多 IDE 账号管理。  
**本 fork 当前交付重点仅 Cursor 无忧切号链**（见 `nirvana-token-20260614`）。
