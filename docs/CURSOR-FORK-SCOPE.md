# Cursor fork 范围说明（对照「你的版本」）

## 当前安装状态（2026-07-15）

| 项 | 值 |
|----|-----|
| 磁盘路径 | `%LocalAppData%\Cockpit Tools\cockpit-tools.exe` |
| SHA-256 | `3374C2853DE9A775686E0F6EADA85B8AEDCB46FC7E019622753F77FB64E2D690` |
| 版本 | ProductVersion `1.3.2` |
| 源码 | `sync-upstream-v1.3.2-20260715` @ `a6efd371`（merge `ebd0dca8`+`a84a97cb` / upstream v1.3.2） |
| Release | `sync-upstream-v1.3.2-20260715`（exe + NSIS） |
| 对照 PR | https://github.com/aliceemoce/cockpit-tools/pull/7 |
| 状态 | **能用**（正式版同步 + Cursor watch 保留 + UIA）；拼装 `1A0EC65E…` **不能用** |

> 合并前锚点：`fork-20260705` @ `918980e9`；Draft `fork-20260705-baseline`（`83E85D7F…`）。

## 历史正式测试版（历史验收基线）

| 项 | 值 |
|----|-----|
| 代号 | `nirvana-merged-20260615` |
| 桌面 exe | `%USERPROFILE%\Desktop\Cockpit-nirvana-token-test.exe` |
| SHA-256 | `F5256511C335ECBCD950B640C6BE04394B00B6CB422FE7F1DC4B1B652D3239DF` |
| 源码 | `aliceemoce/cockpit-tools` 分支 **`agent-build-20260615`** @ `44321ebf` |
| 构建时间 | 2026-06-15 19:03 |
| 历史备份 | `Desktop\Cockpit-nirvana-token-20260614\`（`9791D94F`，仅对照） |
| GitHub 对照 | https://github.com/aliceemoce/cockpit-tools/compare/nirvana-baseline-20260615...agent-build-20260615 |

### 这版包含什么

1. 无忧 Kh **原样 token** 写 profile `state.vscdb`（`token=nirvana_raw`）
2. 无忧切号链：profile 级关进程 → Kh/Gh/Jh/Yh（默认实例 Nc）
3. **每次 Play/多开强制轮换**（`pick_cursor_rotation_account`；用户 2026-06-26 裁决，取代 nirvana「仅满额才换」）
4. **多开** profile 隔离

### 这版不包含什么（nirvana 桌面包对照；非用户全版需求清单）

- **额度池 workos/sub 合并** — **不要**（2026-06-26）
- `cursor_refresh_scheduler` / batch 并发全量刷新 — **不要**（2026-06-26，代码已删/串行）
- UI「会话已过期」与「配额查询失败」分流 — **不要**（2026-06-26，已删）
- 配额 Agent 方案（日限额、transient 分级等）— **待定**
- 默认实例 **全杀 Cursor**（`close_cursor_nirvana_style` 降级）— **不要**（2026-06-26）
> **邮箱去重**：nirvana 桌面包未含；**用户 2026-06-26 裁决：要，已恢复**（`e9d5182f` 口径）。全版 fork 需求以 `user-history-requirements.md` + `FORK-DESIGN-ARCHIVE.md` 为准，本文件仅作 release 对照。

详见 `cockpit-credentials/.../nirvana-token-20260614/VERSION.md`。

---

## 开发仓 `cockpit-tools` 相对你的基线多出来的东西

相对 `eba93695` + patch，**工作区/后续 Agent 提交**可能仍携带下列代码（与桌面测试包**不是同一交付物**）：

| 类别 | 典型符号/文件 | 来源 | 用户裁决（2026-06-26） |
|------|---------------|------|------------------------|
| 额度池合并 | `cursor_quota_pool_key`, 按 pool 去重 | fork `92ec2ebf` | **不要** |
| 导入只认邮箱 | `accounts_are_duplicates` 仅邮箱 | fork `e9d5182f` | **要（已恢复）** |
| 刷新 scheduler | `cursor_refresh_scheduler.rs` | Agent WIP | **不要（已删）** |
| batch 全量 | `refresh_all_tokens_batched` | Agent WIP | **不要（已串行）** |
| UI 会话过期分流 | `isAccountSessionExpired` | fork `92ec2ebf` | **不要（已删）** |
| 每次 Play/多开强制轮换 | `pick_cursor_rotation_account` | 当前 HEAD | **要** |
| 默认实例全杀 Cursor | `close_cursor` → nirvana 降级 | 历史 | **不要（已删降级）** |
| 平台安装徽标 | `PlatformInstalledVersionBadge.tsx` | fork WIP | **待定** |
| Cursor 定时 60min | `default_cursor_auto_refresh` = 60 | 2026-06-14 你明确要求 | **要** |

### 为何会被加进去

1. **`92ec2ebf` 单提交**把 bridge 切号、双开、去重/合并、UI 排序、会话过期展示**打在同一 diff**。
2. **后续 Agent 会话**在配额风暴排查中又加 scheduler、batch、quota-only、验收脚本，未与 `nirvana-token` release 边界对齐。
3. **`AGENTS.md` 旧描述**误写「fork 重点：邮箱去重」，Agent 按错误 brief 继续扩展。

---

## 主仓库 `jlcodes99/cockpit-tools` v0.26.5（对照用，2026-06-22 合并后）

| 项 | 值 |
|----|-----|
| 上游 | `upstream/main` @ `d5ad5cea` |
| 集成分支 | `integrate-upstream-v0.26.5-20260622` @ `92cc6da0` |
| 合并提交 | `c07109f0` merge(upstream): integrate v0.26.5 main into fork |

- 含 upstream v0.25.7→v0.26.5：Claude 平台、Codex session visibility、远程配置等
- 仍保留 fork：无忧切号 / nirvana_raw / 满额换绑 / NVIDIA provider / 多开 pick

## 主仓库 `jlcodes99/cockpit-tools` v0.25.6（历史对照）

- 有 Cursor 账号 **auth_id / 邮箱 / token** 去重（upstream 自带，**不是**你的 release 定义）
- **无**额度池 `cursor_quota_pool_key` 合并
- **无**无忧切号 / nirvana_raw / 满额换绑（fork 独有）

---

## Agent 操作约束

- 说**最新构建**时：默认 SHA `3374C285…` / v1.3.2（能用）；上一 1.3.0=`DEA43F62…`；拼装 `1A0EC65E…` 不能用；合并前锚点 `fork-20260705` @ `918980e9`。
- 说**历史桌面验收包 / nirvana-token 基线**时，默认指 `Cockpit-nirvana-token-test.exe`（SHA `F5256511…`）；二者不要混为一谈。
- 改 Cursor 账号逻辑前：先对照本文件与 `user-history-requirements.md`；**不得**把 scope 对照表当作用户需求源。
- 文档以 **历史要求 + 设计档案** 为全版目标；本文件仅标注 nirvana 桌面包对照与用户已裁决项。
