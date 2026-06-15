# Cursor fork 范围说明（对照「你的版本」）

## 你的正式测试版（唯一验收基线）

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
3. **满额自动换号**（Play / 多开启动时绑定未满则换满额号）
4. **多开** profile 隔离

### 这版不包含什么（勿写进 release 说明）

- Cursor **邮箱去重 / 导入只认邮箱**（非本 release 需求）
- **额度池 workos/sub 合并**（`cursor_quota_pool_key` 等，非本 release 需求）
- `cursor_refresh_scheduler` / batch 并发全量刷新
- UI「会话已过期」与「配额查询失败」分流（`isAccountSessionExpired`）
- 配额 Agent 方案（日限额、transient 分级等）

详见 `cockpit-credentials/.../nirvana-token-20260614/VERSION.md`。

---

## 开发仓 `cockpit-tools` 相对你的基线多出来的东西

相对 `eba93695` + patch，**工作区/后续 Agent 提交**可能仍携带下列代码（与桌面测试包**不是同一交付物**）：

| 类别 | 典型符号/文件 | 来源 | 你是否要求 |
|------|---------------|------|------------|
| 额度池合并 | `cursor_quota_pool_key`, `normalize_account_index` 按 pool 去重 | fork 提交 `92ec2ebf` / `cd022242` | **否** |
| 导入只认邮箱 | `禁止非邮箱去重` 注释块 | fork `e9d5182f` 等 | **否** |
| 刷新 scheduler | `cursor_refresh_scheduler.rs`（工作区已删，HEAD 或有） | Agent WIP | **否** |
| batch 全量 | `refresh_all_tokens_batched`（工作区已改回串行） | Agent WIP | **否** |
| UI 会话过期分流 | `isAccountSessionExpired`, CursorAccountsPage 双徽标 | fork `92ec2ebf` | **否** |
| 平台安装徽标 | `PlatformInstalledVersionBadge.tsx` | fork WIP | **否** |
| Cursor 定时 60min | `default_cursor_auto_refresh` = 60 | 2026-06-14 你明确要求 | **是** |

### 为何会被加进去

1. **`92ec2ebf` 单提交**把 bridge 切号、双开、去重/合并、UI 排序、会话过期展示**打在同一 diff**。
2. **后续 Agent 会话**在配额风暴排查中又加 scheduler、batch、quota-only、验收脚本，未与 `nirvana-token` release 边界对齐。
3. **`AGENTS.md` 旧描述**误写「fork 重点：邮箱去重」，Agent 按错误 brief 继续扩展。

---

## 主仓库 `jlcodes99/cockpit-tools` v0.25.6（对照用）

- 有 Cursor 账号 **auth_id / 邮箱 / token** 去重（upstream 自带，**不是**你的 release 定义）
- **无**额度池 `cursor_quota_pool_key` 合并
- **无**无忧切号 / nirvana_raw / 满额换绑（fork 独有）

---

## Agent 操作约束

- **验收与部署默认用桌面** `Cockpit-nirvana-token-test.exe`，不要与 `%LocalAppData%` 里 Agent 随手编译的 exe 混为一谈。
- 改 Cursor 账号逻辑前：先对照本文件与 `source-diff-uncommitted.patch`，**不得**把「邮箱去重 / 额度池合并 / 会话过期 UI」当 release 需求加回去。
- 文档以 **nirvana-token 范围** 为准；开发仓多出的模块应标注「未纳入 release / 待还原 upstream」。
