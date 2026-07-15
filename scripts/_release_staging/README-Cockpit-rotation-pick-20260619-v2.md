# Cockpit 多开挑号修复 v2 — 对照说明（相对 BC99）

## 包信息

| 项 | BC99（上一候选） | 本包 v2（当前可用） |
|----|------------------|---------------------|
| 文件名 | `Cockpit-nirvana-token-test-multi-close-fix-20260617.exe` | `Cockpit-rotation-pick-20260619-v2.exe` |
| SHA-256 | `BC99ED00E892F58BD43DF666C0286ED46FA808225F306C544F58F07CA15C30B8` | `F17C961FF725D44C355578F7FB7EE3103FD35423D02086DFCE018483D0DE964A` |
| 源码对照基线 | Git 提交 `60efa8ca`（BC99 源码快照） | 工作区相对 `60efa8ca` 的未提交改动 + 3 个新文件 |
| GUI 验收 | 多开仍走「自动挑选满额账号」旧链，可能选到坏号 | `verify_cockpit_gui_multi_start.py` → **`ok: true`**；日志含 `pick: pool=full`、`自动轮换账号`，**不含** `自动挑选满额账号` |

**重要说明**：v2 是用**整个当前工作区**编译出来的，不只包含下面「多开挑号」相关改动；工作区里还有 Codex、Windsurf、cliproxy 等其它文件的 diff，也一并进了这个 exe。若你只要多开挑号修复，应只看下文「一、本次任务相关改动」；「二、同包内但非本任务」列出可能多余的部分。

---

## 一、本次任务相关改动（多开不写坏号）

对照基线：Git `60efa8ca` 上的 [`cursor_account.rs`](../../src-tauri/src/modules/cursor_account.rs) 与 [`cursor_instance.rs`](../../src-tauri/src/commands/cursor_instance.rs)。

### 1. 挑号规则：与账号总览同一套，不再自造一套

**BC99 怎么做**

- 多开点「启动」时：若当前绑定号「额度未满」，就日志写「绑定账号额度未满，自动挑选满额账号」，然后调用 `pick_full_quota_account`。
- `pick_full_quota_account` 用 `average_quota_percentage`（各维度额度平均值）判断是否满额，**不要求**与账号总览列表上的绿条/红条一致。
- 满额池里**不**按账号总览的「会话过期 / 配额失败」规则挡号；refresh 已报「会话过期」的号仍可能因「平均值满额」被选中（例如 `cursor_27d0fb3e88f9` 写进默认实例 → 登录页）。

**v2 怎么做**

- 新增「镜像账号总览」的三个概念（Rust 函数名仅供对照，行为对齐前端）：
  - **异常号**（对齐 `isAbnormalAccount`）：封禁、`status=error`、或 `quota_query_last_error` 里命中「会话过期 / 未认证 / 请重新导入」等关键词 → **永不进多开 pick 池**。
  - **剩余额度**（对齐 `resolveRemainingQuotaPercent`）：磁盘上有 `quota_query_last_error` → 视为无额度（`null`）；否则 `100 - max(total/auto/api 已用%)`；剩余 ≤0 → **不进池**。
  - **可切号 token**：与写进 Cursor 的 inject 路径一致——`cursor_auth_raw` 为空时**回退**顶层 `access_token`（BC99 的 pick 门槛曾要求 raw 里双 token，导致 965 个仅有 `access_token` 的号进不了池）。
- 新函数 `pick_cursor_rotation_account`：
  - **满额池**（remaining ≥ 99）：池内**均匀随机**，不再固定选排序第一名。
  - **好号池**（1～98%）：仅满额池空时使用，按 remaining **加权随机**（额度越高越易被选中，与总览「按剩余额度从高到低排」一致）。
  - 日志固定打：`pick: pool=full|good`、`candidates`、`picked`、`remaining=%`。
- 删除多开路径上的旧日志「已因额度不足自动改绑满额账号 / 自动挑选满额账号」；改为「自动轮换账号: … pool=full, remaining=…%」。

### 2. 写盘前：现场 probe，失败换号

**BC99**

- 挑完号直接 inject + 启动，**不**对即将写入 Cursor 的 token 做 API 探测。
- 写盘校验只看 vscdb 里有没有 token 字符串，**不**验证 Cursor 服务端是否认这个 session。

**v2**

- 多开自动挑号改为循环：`pick` → **最多 3 次** `probe_cursor_account_live_auth`（用与 inject 相同的 `nirvana_kh_auth_tokens` 取 access）→ 通过才 inject。
- probe 失败：该 account_id 加入 exclude，**重新 pick**；池空则报错，**不启动**。
- 删掉了「网络抖动 transient 仍放行 inject」的分支（BC99 无此链；中间版本曾误加）。

### 3. 行内 Play（账号总览点某一行的 Play）

**BC99**：用 `ensure_cursor_switch_ready_account`（另一套「可切号」判定）。

**v2**：改用 `ensure_cursor_overview_pickable`（与多开 pick **同一套**总览规则）；失败直接报错，不写盘。

### 4. 切号审计日志（新文件，便于对照坏号）

**新增** [`cursor_switch_audit.rs`](../../src-tauri/src/modules/cursor_switch_audit.rs)，写入 `%USERPROFILE%\.antigravity_cockpit\logs\cursor_switch_audit.jsonl`：

- 阶段：`switch_start` → `pick` → `probe_pre` → `inject` → `launch` → `probe_post`。
- 每条带 `switch_trace_id`，可与 `app.log` 同一时间线对照。

**附带小改**

- 标记 `quota_query_last_error` 写盘时多打 WARN，并写 audit `ui_error_mark`。
- `is_cursor_auth_quota_error` 关键词与前端 `accountValidityFilter.ts` 对齐（补「凭证无效」「授权已失效」等）。

### 5. 验收脚本（新文件，Agent 自用）

| 文件 | 作用 |
|------|------|
| `verify_cockpit_gui_multi_start.py` | 启动 **COCKPIT_EXE 指定包** → UIA 点多开「启动」→ 查日志增量 + vscdb |
| `verify_multi_instance_switch.py` | CLI `dual-cursor-launch` 辅助自检（**不能代替 GUI 验收**） |

v2 实测：`cockpit_gui_multi_start_report.json` 中 `log_has_pick_pool_full=true`、`log_has_rotation=true`、`log_has_old_pick=false`、`ok=true`。

### 6. 单元测试（仅 Rust 测试，不进 exe 行为）

- `cursor_overview_pick_tests`：Auto 100% 总用量 → remaining=0 不可 pick；0% used → 100% 可进满额池；会话过期 → abnormal。
- `switch_ready_when_raw_empty_but_access_token_present`：raw 空对象 + 顶层 token → 可 pick。

---

## 二、同包内但非「多开挑号」任务的改动（可能多余）

以下 diff 相对 `60efa8ca` **也打进了 v2 exe**，与多开挑号无直接关系；若你要最小 diff，后续可只 cherry-pick「一」中文件重新编译。

| 区域 | 文件（举例） | 大致内容 |
|------|--------------|----------|
| Codex | `codex.rs`、`codex_instance.rs`、`codex_local_access.rs`、`codex_session_visibility.rs`、`codex_speed.rs` | Codex 实例/本地访问/会话可见性等小改 |
| Token 保活 | `provider_token_keeper.rs` | Cursor refresh 失败时的日志/保活文案 |
| 账号总览 Play 后处理 | `cursor.rs` | 切号成功日志改为「以 probe_post 为准」；**去掉**切号成功后立刻 spawn 一次 `refresh_account_fast_async` |
| 对齐/备份 | `cursor_switch_align.rs`、`cursor_backup_token_embedded.rs`、`cursor_import_backup_sync.rs` | 注释或极小逻辑调整 |
| WebDAV / 系统 | `webdav_sync.rs`、`data_transfer.rs`、`system.rs` | 同步或系统命令小改 |
| HTTP | `utils/http.rs` | 几行级别 |
| 其它 IDE | `vscode_inject.rs`、`windsurf_devin_oauth.rs`、`cockpit-core` 下 account 模型 | 非 Cursor 多开 |
| Sidecar | `sidecars/cockpit-cliproxy/main.go` | cliproxy 约 +84 行 |
| CLI | `crates/cockpit-cli/src/main.rs` | cockpit 命令行小改 |
| 验收脚本 | `verify_dual_cursor_instances.py` | 双实例验收脚本调整（非 v2 核心） |
| Agent 文档 | `.cursor/project-brief.md`、特征注册表 | 文档，不进 exe |

**结论**：多开挑号核心只有 **`cursor_account.rs` + `cursor_instance.rs` + 新 `cursor_switch_audit.rs` + `mod.rs` 注册**；其余 20+ 文件是工作区里其它并行改动，**不是**为「多开不写坏号」单独设计的，但当前 v2 包**包含**它们。

---

## 三、行为对比（一句话）

| 场景 | BC99 | v2 |
|------|------|-----|
| 多开点启动 | 绑定未满额 → 挑「平均值满额」号，可能选 refresh 已失败的坏号 | 排除异常/无额度/无 token → 满额池随机 → probe 通过才写盘 |
| 与总览列表 | 排序指标不一致（average vs 剩余%） | remaining 与总览 `resolveRemainingQuotaPercent` 同一算法 |
| 日志关键字 | `自动挑选满额账号` | `pick: pool=full` + `自动轮换账号` |

---

## 四、附件与线上一览

- 本说明 + exe：GitHub Release `cockpit-rotation-pick-20260619-v2`
- 完整 diff 补丁：`diff-full-vs-bc99.patch`（工作区 vs `60efa8ca`）
- 仅多开相关补丁：`diff-cursor-multi-pick.patch`
- 在线逐行对照：GitHub Compare `60efa8ca...cockpit-rotation-pick-20260619-v2`（推送分支后）

## 五、验收命令（复现 GUI 通过）

```powershell
$env:COCKPIT_EXE = "...\scripts\_release_staging\Cockpit-rotation-pick-20260619-v2.exe"
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--force-renderer-accessibility"
python scripts/verify_cockpit_gui_multi_start.py
```

通过时 `cockpit_gui_multi_start_report.json` 中 `ok: true`，且 `expected_exe_sha256` 以 `f17c961f` 开头。
