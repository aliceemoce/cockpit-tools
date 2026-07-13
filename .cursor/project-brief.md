# 会话快照 — 2026-07-14

## 工作区身份
- **项目**：Cockpit Tools fork（Tauri 2 + React 19 + Rust）
- **路径**：`C:\Users\aliceemoce\dev\cockpit-tools`
- **当前分支（本地名）**：`sync-upstream-v1.3.0-20260714` → 跟踪/推送 **`origin/sync-upstream-v1.3.0-20260714-full`**
- **HEAD**：`102904ee`
- **仓库**：https://github.com/aliceemoce/cockpit-tools

## 📋 项目理解（本会话）
主仓正式 release **v1.3.0** 触发 F-007/F-008 同步。第一轮交付不合格（删上游 UI + debug 当基线）。第二轮已完整合入上游前端、release 覆盖安装 SHA **`1A0EC65E…`**、推送 `-full`、Release 资产与 PR #5；正在回写规则并将网页复核收尾。Cursor fork 边界（不全杀切号、多开、强制轮换、Kh、transient、邮箱去重）保留；额度池/scheduler/会话过期分流不带回。

## 安装 exe 状态（磁盘实测 2026-07-14）
| 项 | 值 |
|---|---|
| 路径 | `%LocalAppData%\Cockpit Tools\cockpit-tools.exe` |
| SHA-256 | `1A0EC65E5672615BAC8A0DEBCFB1B6AD15DB546DE17545BAE1173060CD936DC5` |
| ProductVersion | **1.3.0** |
| 形态 | **release**（debug `09BA38D3…` 已撤销） |

## GitHub 交付
| 项 | URL / 值 |
|---|---|
| 分支 | `sync-upstream-v1.3.0-20260714-full` @ `102904ee` |
| 对照 PR | https://github.com/aliceemoce/cockpit-tools/pull/5 |
| Release | https://github.com/aliceemoce/cockpit-tools/releases/tag/sync-upstream-v1.3.0-20260714 |
| 资产 | `cockpit-tools.exe`（digest=上述 SHA）；NSIS 未齐 |

## 待收尾
- [x] release 构建 + 覆盖安装 + UI 非网络错误
- [x] Release 资产 SHA 对齐；删误导 setup
- [x] 规则/设计档案/AGENTS/HR commit+push 到 `-full`（`13c0add0`）
- [x] 系统浏览器 + UIA 复核 PR #5 / release / `-full` 分支页
