# Cursor 换号逻辑 & 多开实例差异

## 换号是不是「无忧小助手」逻辑？

**不是。** 全仓库无 `无忧` / `wuyou` / `Wuyou` 字符串。

Cockpit 自有实现：

| 步骤 | 账号总览「切换到 Cursor」 | 多开实例「启动」 |
|------|---------------------------|------------------|
| 入口 | `inject_cursor_account` | `cursor_start_instance` → `inject_bound_account_for_instance_start` |
| 关进程 | 关默认 `--user-data-dir` | 关目标实例目录 |
| 指纹重置 | `hard_reset_cursor_fingerprint_state`（默认 profile） | **同上（按实例 profile 目录）** — 2026-06-08 已与总览对齐 |
| 写 token | `inject_to_cursor` → 默认 `state.vscdb` | `inject_account_to_profile` → 实例 `state.vscdb` |
| 当前账号 | 写 `provider_current_state` | 仅绑定账号时注入 |
| 启动 | `cursor_start_instance("__default__")` | `start_cursor_with_args` + `--user-data-dir` |

指纹重置会改写 profile 下 `storage.json` / `machineId` / `state.vscdb` 中的 telemetry 字段（日志称「n 风格」，指 Cockpit 内置重置流程，非第三方助手）。

## 为何多开实例「无法登录」？

常见原因：

1. **未绑定账号** — 实例无 `bind_account_id` 时只启动空 profile，需先在实例卡片绑定账号。
2. **实例未初始化** — 绑定前须至少启动一次生成 `User/globalStorage/state.vscdb`（或勾选复制当前登录状态）。
3. **此前缺指纹重置** — 仅写 token 不重置指纹时，Cursor 可能仍用旧会话；已在实例注入前补上 `hard_reset_cursor_fingerprint_state_for_profile`。

账号总览切换始终针对**默认 profile**；多开使用**独立 user-data-dir**，逻辑路径不同但注入步骤现已一致。
