# 会话快照 — 2026-07-05

## 工作区身份
- **项目**：Cockpit Tools fork（Tauri 2 + React 19 + Rust）
- **路径**：`C:\Users\aliceemoce\dev\cockpit-tools`
- **Git 分支**：`fork-20260705`（禁止往 `integrate-upstream-v0.26.5-20260622` push 新提交）
- **本地 HEAD**：`2eac8fff`（源码快照，对应安装 exe SHA `141CB61C`）；**远程尚未有 `origin/fork-20260705`**
- **仓库**：https://github.com/aliceemoce/cockpit-tools

## 安装 exe 状态（磁盘实测 2026-07-05）
| 项 | 值 |
|---|---|
| 路径 | `%LocalAppData%\Cockpit Tools\cockpit-tools.exe` |
| SHA-256 前缀 | `141CB61CAE1D3757…` |
| 修改时间 | 2026-07-02 01:44 |
| 进程 | 当前未运行 |

> 特征注册表仍记 `71E052FB…`（2026-06-23 NSIS）为「能用」锚点；本机实际安装包已更新为 7/2 构建，**注册表与 brief 需增量同步**。

## Cursor 运行时数据
| 项 | 值 |
|---|---|
| 账号 JSON 目录 | `%USERPROFILE%\.antigravity_cockpit\cursor_accounts\` |
| 邮箱 JSON 数 | **2571**（2026-07-05 计数） |
| 错误路径（禁止当池） | `data\cursor_accounts\` |

## 工作区未提交改动（13 文件，+297/-79）
相对 `3320d442` 的**源码层**变更，**尚未 commit/push**：

### 后端 `cursor_account.rs` / `cursor_instance.rs`
- 用户可见配额失败文案统一为「配额查询失败」；禁止「会话已过期/失效」（HR-20260701-004）
- `refresh_for_forced_account_switch`：手动选号不因 pending/配额失败/封禁阻断，仅缺 token 硬拦（HR-20260701-005）
- `ensure_cursor_manual_pick_allowed` / `manual_user_pick` 分支贯穿切号链
- `nirvana_traditional_switch_steps` 增加 manual 参数

### 前端 `CursorAccountsPage.tsx` + `cursor.ts` + locales
- tier-badge：pending 显示「配额未查询」，隐藏红色 UNKNOWN（HR-20260701-003）
- 去掉顶栏「待查询配额」status-pill；配额区保留 pending 占位
- 配额错误区显示「配额查询失败」

### 规则 / HR（亦未提交）
- `.cursor/user-history-requirements.md` 追加 HR-20260701-001～005、HR-20260702-001～002
- `.cursor/rules/cockpit-cursor-data-paths.mdc`（新，未跟踪）
- 特征注册表、banned-phrases 小改

## 近期已执行（脚本证据，非 git）
| 任务 | 证据 | 状态 |
|---|---|---|
| 32 桌面邮箱上传远端 | `scripts/_upload_desk32_report.json`：uploaded 32/32 → `aliceemoce/cockpit-credentials` | 已完成 |
| 桌面邮箱合并运行时 | `_desk_only_online_audit.json`：32/32 in_runtime | 已完成 |
| 今日 Cockpit 自动备份核对 | `_cockpit_backup_today_report.json`（2026-07-02）：local 19 条非 desk32 | 已跑 |
| 角标/UI 构建 | `_tauri_build_badge_fix_20260701.txt`、截图 `cockpit_no_unknown_badge_20260702.png` | 有构建记录 |

## 未完成 / 风险
1. **工作区 Cursor 改动未 commit/push** — 与安装 exe 是否含这些改动需 SHA 对照或重编译验收
2. **UI 验收** — 7/1～7/2 有截图留证，本会话未重跑 MCP 画面核对
3. **设计档案** — `docs/FORK-DESIGN-ARCHIVE.md` 最后更新 2026-06-26，未反映 7/1 角标与手动选号改动
4. **scripts/** — 大量 `_` 前缀临时脚本/截图/exe staging **未纳入 git**（符合 deploy 规则）
5. **三件套** — 无 `workspace-purpose/plan/design.md`；等价映射：`AGENTS.md` + HR 主档 + 特征注册表

## 当前活跃历史要求（摘要）
- **HR-20260701-003～005**：角标「配额未查询」、禁「会话已失效」文案、手动 Play 不限
- **HR-20260702-001～002**：正确路径规则、桌面邮箱合并、32 上传 + 今日备份核对
- **HR-20260630-009**：新 fork 改动须 push 到 `fork-YYYYMMDD`，禁止旧 integrate 分支
- **部署硬约束**：仅 `npm run build` + `npm run tauri build` 可覆盖安装；禁止裸 debug exe

## 本会话角色
只读工作区汇报助手；仅维护本 brief 与 `.brief-ready`，不改源码/配置。
