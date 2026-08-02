# Cockpit Tools Fork Persistent Design Archive

**最后更新**: 2026-07-19  
**维护人**: 用户 + Codex/Cursor Agent  
**性质**: 这个 fork 的长期功能设计档案，用来防止后续修改覆盖、回退或误解用户目标。

## 1. 档案用途

这份档案记录“用户为什么要这个 fork 的功能”以及“代码现在如何实现这些功能”。它不是普通项目介绍，也不是一次性总结。任何手动 fork、自动化任务、代码修改、文档修改开始前都要先读；只要本次实际修改了内容，结束后都要按功能增量维护。

如果自然语言目标和代码实现发生冲突，以用户目标为准；代码设计可以换实现手段，自然语言设计不能被实现细节改成另一个需求。

## 2. 维护规则

- 修改前先读 `AGENTS.md`、`docs/CURSOR-FORK-SCOPE.md`、`.codex/rules/fork-design-archive-maintenance.md`、`.cursor/project-brief.md`、本档案和特征注册表。
- 修改后只要内容发生变化，就维护本档案；默认增量追加或小范围修订，不整篇重写。
- 自然语言设计只写用户目的、边界和验收含义，保持简洁明确，不把实现手段写成目标。
- 代码设计写实现手段、模块位置、依赖选择、验证方式和风险。
- 新增或替换实现方案前，优先通过项目 MCP、upstream、官方文档、成熟库或原作者做法确认，不自造私有术语或私有流程。
- 如果只做巡检且没有发生代码、文档、构建、发布变化，不需要改本档案。

## 3. Fork 总目标

这个 fork 的核心目的，是保留用户已经认可的 Cursor 无忧切号链，并在合并 upstream 正式版本更新时不破坏这条可用链路。

当前 release 范围不包括邮箱去重、额度池合并、刷新 scheduler/batch、配额 Agent 方案，除非用户明确重新要求。

## 4. 持久功能档案

### F-001 Cursor 默认实例切号不全杀

**用户目的**: 默认实例切号时，只处理需要切换的默认 Cursor，不破坏用户其它正在使用的 Cursor 进程。

**自然语言设计**: 切号应该像“换当前默认账号”，不是“关闭所有 Cursor”。用户同时开的其它窗口、其它 profile、其它工作上下文要尽量保留。

**代码设计**: 默认实例走只影响默认 profile 的关闭/切换路径；多开 profile 使用更严格的 profile 级关闭路径。相关实现集中在 `src-tauri/src/modules/cursor_instance.rs`、`src-tauri/src/commands/cursor_instance.rs` 以及 core 侧对应逻辑。

**保留边界**: 不允许把默认 Play 路径改回全局 `taskkill` 或 `close_cursor_nirvana_style` 作为默认行为。

**验证方式**: UIA 或脚本验证默认实例切号后，其它 Cursor profile 仍可同时存在。

### F-002 Cursor 多开 profile 隔离

**用户目的**: 允许多个 Cursor 实例各自绑定账号和 profile，不互相污染。

**自然语言设计**: 默认实例和多开实例可以同时存在；多开应该隔离 profile，而不是把所有账号和窗口混成一个状态。

**代码设计**: 多开实例使用 profile 级路径、profile 级关闭和启动逻辑；账号绑定、实例记录和 UI 展示要围绕 profile 身份工作。相关实现集中在 Cursor instance 模块、账号模块和 `src/pages/CursorAccountsPage.tsx`。

**保留边界**: 不允许为了简化实现把多开退回单实例账号切换。

**验证方式**: 启动默认实例和至少一个多开实例，确认进程/profile/账号绑定互不覆盖。

### F-003 满额账号优先绑定

**用户目的**: 启动或切号时优先使用可用的满额账号，减少手动挑号。

**自然语言设计**: 自动换号是为了优先选可用账号，不是每次都强制轮换，也不是把所有账号做复杂池化。

**代码设计**: 使用现有账号排序与 `pick_full_quota_account` 相关逻辑；保留 Kh 原样 token 链路，避免把可用 token 二次加工到不可用。

**保留边界**: 不做用户未要求的额度池合并、邮箱去重、强制轮换或 batch 全量刷新。

**验证方式**: 对照账号列表、绑定结果和 UI 展示，确认选中账号符合可用/满额预期。

### F-004 transient 失败不写成账号失效

**用户目的**: 网络、超时、临时失败不能把账号误标成不可用或会话过期。

**自然语言设计**: 临时失败要被当成临时失败，不能污染账号状态；只有明确 auth 类失败才可以写入失效信息。

**代码设计**: 保留 `is_cursor_transient_quota_error` 等 transient 分类，避免无条件写 `quota_query_last_error`。相关逻辑在 Cursor 账号、quota 查询和配置写入链路。

**保留边界**: 不允许恢复 `7761f453` 式全局监听或无条件写盘行为。

**验证方式**: 模拟或观察网络/超时失败，确认不会写入误导性失效状态。

### F-005 GUI 验收以真实桌面为准

**用户目的**: 验证结果要证明用户真正打开的 Cockpit UI 没搞错，而不是脚本自说自话。

**自然语言设计**: UI 验收要看真实桌面窗口和可交互控件；不能用内置浏览器、坐标鼠标或命令行输出替代真实 UI 结果。

**代码设计**: 验收优先走 `uia-pattern` 的 `ui_invoke`，必要时用只读 UIA 工具观察窗口。构建验证复用仓库已有脚本和 Tauri/Rust/前端标准命令。

**保留边界**: 不打开或引入带命令行闪屏的实验性入口；不把命令行实验内容带进 GUI 验收路径。

**验证方式**: 打开桌面应用，用 UIA 检查关键页面、按钮和行为，无命令行窗口或实验入口误入。

**对用户结论**: 仅 **已修好** / **没修好**（见 `.cursor/rules/honest-fix-status-only.mdc`）；禁止「仅后端已验收」等分拆话术冒充修好。

### F-006 构建基线和验收基线必须区分

**用户目的**: 不再把历史桌面验收包、当前最新运行构建、开发分支源码混为一谈。

**自然语言设计**: 用户说“最新构建/当前运行”默认指 `%LocalAppData%\Cockpit Tools\cockpit-tools.exe`；用户说“历史桌面验收包/nirvana-token 基线”才指桌面的 `Cockpit-nirvana-token-test.exe`。

**代码设计**: 版本记录和规则文档必须写明路径、SHA、分支、upstream 基线。发布或更新后同步维护特征注册表、范围文档和本档案。

**保留边界**: 未经用户说明，不允许把当前最新构建回退到历史验收包。

**验证方式**: 用文件路径、SHA、分支和 GitHub 页面互相校验。

### F-007 upstream 同步只响应正式版本更新

**用户目的**: 自动化只在主仓发布新版本时合并，不因为普通提交、公告、广告开关或文案变化打扰稳定 fork。

**自然语言设计**: “发布新更新”指 release/tag/版本号变化，不是 `main` 有新提交。若没有正式版本更新，任务立即停止，不做动作。

**代码设计**: 定时任务检查 upstream latest release、最新 tag 和版本字段；只有版本高于已记录集成版本才进入合并流程。合并起点使用本地最新可用分支（当前默认 `sync-upstream-v1.3.10-20260719` / 已记 **upstream v1.3.10 → 交付 ProductVersion 1.3.14**；上一项正式同步为 v1.3.8→1.3.13），保留 fork 功能（含 `sync_cursor_local_watch`）和 release 边界。

**保留边界**: `announcements.json`、广告位开关、公告配置、普通 commit 不触发同步、构建、发布或文档改动。

**验证方式**: 对照 GitHub release/tag/version 字段；若触发合并，必须用 PR 做线上对照。

### F-008 upstream 同步交付链

**用户目的**: 合并 upstream 时，先保证合并前版本的代码和 exe 已在个人 GitHub 仓库可追溯；合并后代码和 exe 也要上传，并通过网页确认存在。

**自然语言设计**: 每次正式版本同步都要可回看、可对照、可回退。代码和 exe 都不能只留在本机。

**代码设计**: 自动化以个人仓库 `aliceemoce/cockpit-tools` 为交付位置；合并前检查分支/提交和 release 资产，合并后推送同步分支、创建 PR 对照、上传 exe release asset，并更新最新构建记录。

**保留边界**: 未确认合并前代码和 exe 已存在时，不继续合并；发布后未网页复核时，不宣称完成。

**验证方式**: 系统浏览器 + UIA 打开个人仓库分支页、PR/compare 页和 release 资产页，确认代码与 exe 都存在。

### F-009 设计档案持续维护

**用户目的**: 让用户和 Agent 都能理解 fork 中每个功能为什么存在、怎么实现，防止后续任务顾此失彼。

**自然语言设计**: 档案按功能维护，不按一次性任务堆说明；用户目标是稳定锚点，实现手段可以演进。

**代码设计**: `AGENTS.md` 是 Codex 也会读取的硬入口；`.codex/rules/fork-design-archive-maintenance.md` 是 Codex 侧规则镜像；`.cursor/rules/fork-design-archive-maintenance.mdc` 是 Cursor 侧规则镜像；自动化 prompt 也必须显式要求读写本档案。

**保留边界**: 不能只把要求写在 Cursor 规则里就当 Codex 已遵守；也不能只放一个 `.codex/` 文件就假设自动生效。Codex 必须通过 `AGENTS.md`、自动化 prompt 或本轮实际读取来获得约束。

**验证方式**: 每次改动后检查本档案、`AGENTS.md`、相关规则和自动化 prompt 是否仍一致。

### F-010 Cursor 本地当前账号自动入库与 current 同步

**用户目的**: 用户在 Cursor 默认 profile 里换号后，Cockpit 应自动把该账号写入账号库，并把总览「当前账号」对齐到本地登录态；长驻 Cockpit 窗口应能刷新列表与 current，不必重启电脑。

**自然语言设计**: 默认 profile 的 `state.vscdb` 是「真实当前号」来源；Cockpit 列表与 `provider_current_accounts.json` 的 cursor 字段须与此一致；仅后端写盘、界面仍空白或搜不到，不算完成。

**代码设计**:
- `provider_token_keeper::sync_cursor_local_watch` 每 20s 调用 `cursor_account::sync_local_cursor_from_default_profile`。
- 读 `read_local_cursor_auth()` → 必要时 `upsert_account_with_outcome` + `record_import_backup` → `set_current_account_id("cursor")` → `accounts:changed`（`local-auto-import` / `local-current-sync`）。
- 前端 `useProviderAccountsPage` 监听 `accounts:changed` 后 `fetchAccounts()`（含 current）。

**保留边界**: 不恢复多开 `sync_sidebar_state_from_default_to_profile`；多开 bind/current 可与总览 current 不同。

**验证方式**: 磁盘 `cursor_accounts.json` + `provider_current_accounts.json` 与 Cursor 本地邮箱一致；**且** MCP 证明 Cockpit WebView 非「localhost 网络错误」、Cursor 账号页可打开。

### F-011 桌面构建与部署链

**用户目的**: 交付给用户的 exe 必须能打开完整前端，不能把「编译通过」或「磁盘脚本通过」冒充「可用」。

**自然语言设计**: 覆盖 `%LocalAppData%\Cockpit Tools\cockpit-tools.exe` 的构建须走完整 Tauri 前端打包链；部署后 Agent 必须亲自 UIA 验收 WebView 与目标页。

**代码设计**:
- 标准命令：`npm run tauri build`（执行 `beforeBuildCommand`: `prepare-tauri.cjs` + `npm run build`，再 `cargo` release 并嵌入 `../dist`）。
- **禁止**仅 `src-tauri` 下单独 `cargo build --release` 后复制 exe 当作已交付版本（曾出现 WebView 连 `localhost:1420`、界面「网络错误」而后端仍写盘）。
- NSIS 签名失败（缺 `TAURI_SIGNING_PRIVATE_KEY`）时，仍可使用 `target/release/cockpit-tools.exe` 覆盖安装，但须完成 F-005 UI 验收。

**保留边界**: 不得跳过 UI 验收宣称「已修复」；后端与 UI 验收结论须分开写。

**验证方式**: 安装 exe SHA 与 `target/release/cockpit-tools.exe` 一致；MCP `list_windows` → WebView 标题含应用内容而非「localhost - 网络错误」。

## 5. 增量记录格式

每次实际修改后追加一条，或在对应功能卡做最小修订：

- **日期**
- **触碰功能**
- **用户目标是否变化**
- **本次目的**
- **实现手段**
- **涉及文件/模块**
- **验证方式**
- **风险或注意事项**

## 6. 增量记录

### 2026-06-24

- **触碰功能**: F-009 设计档案持续维护。
- **用户目标是否变化**: 是，用户明确要求档案必须覆盖所有 fork 功能和目的，并在手动/自动化任务前后维护。
- **本次目的**: 建立持久功能设计档案，防止后续修改覆盖、回退或误解用户原始目标。
- **实现手段**: 新增本档案，按功能卡记录自然语言设计、代码设计、保留边界和验证方式，并接入 `AGENTS.md`、`.cursor/rules/`、自动化 prompt。
- **涉及文件/模块**: `docs/FORK-DESIGN-ARCHIVE.md`、`AGENTS.md`、`.cursor/project-brief.md`、`.cursor/rules/fork-design-archive-maintenance.mdc`、`.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc`、Codex automation `cockpit-upstream-sync-watch`。
- **验证方式**: 读取相关文件确认规则入口存在；查看 automation 配置确认独立定时任务会先读并在实际变更后维护本档案。
- **风险或注意事项**: `.cursor/rules` 对 Codex 不是自动生效机制，Codex 侧必须依赖 `AGENTS.md`、自动化 prompt 和实际读取。

### 2026-06-24

- **触碰功能**: F-007 upstream 同步只响应正式版本更新。
- **用户目标是否变化**: 否，属于对“发布新更新”含义的纠偏。
- **本次目的**: 把 upstream 同步自动化从普通提交触发改成正式 release/tag/version 触发。
- **实现手段**: 自动化只检查 latest release、最新 tag 和版本字段；普通 `main` 提交、`announcements.json`、公告/广告配置不触发整套流程。
- **涉及文件/模块**: Codex automation `cockpit-upstream-sync-watch`。
- **验证方式**: 查看 automation prompt，确认无新版本发布时停止且不做代码、构建、发布或文档改动。
- **风险或注意事项**: 后续如果 upstream 改变发版方式，需要先更新 F-007 的代码设计，再改自动化判断。

### 2026-06-24

- **触碰功能**: F-009 设计档案持续维护。
- **用户目标是否变化**: 否，属于把同一要求补到 Codex 侧入口。
- **本次目的**: 不只依赖 `.cursor/rules`，为 Codex 增加明确的规则镜像和启动读取入口。
- **实现手段**: 新增 `.codex/rules/fork-design-archive-maintenance.md`，并把它写入 `AGENTS.md` 启动协议、硬约束和本档案 F-009。
- **涉及文件/模块**: `.codex/rules/fork-design-archive-maintenance.md`、`AGENTS.md`、`docs/FORK-DESIGN-ARCHIVE.md`、`.cursor/rules/fork-design-archive-maintenance.mdc`、`.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc`、Codex automation `cockpit-upstream-sync-watch`。
- **验证方式**: 检查 `.codex/rules/` 文件存在，确认 `AGENTS.md` 和自动化 prompt 都显式要求读取 Codex 侧规则。
- **风险或注意事项**: `.codex/rules/` 本身仍不是系统级强制机制；必须由 `AGENTS.md` 和自动化 prompt 显式引用。

### 2026-06-24

- **触碰功能**: Cursor 账号页排序与配额状态展示（扩展：待查询配额 + 导入后自动查询）。
- **用户目标是否变化**: 否（按用户新增验收口径补齐“未查过配额”的可见状态与导入后自动拉取）。
- **本次目的**:
  - 未查过配额的账号在 UI 显示「待查询配额」，并在排序中沉底（与查询失败区分）。
  - 新导入账号落盘后立刻触发配额查询（不依赖用户手动刷新，也不受账号数量影响）。
  - 编译后必须验证 Cockpit WebView 正常加载，并执行一次全量配额刷新以刷新磁盘与 UI。
- **实现手段**:
  - 前端增加 `isCursorQuotaPendingQuery` 判定与文案 `pendingQuery`，在卡片/表格上展示角标和占位文案；排序分层：当前账号优先 → 失败沉底 → 待查询沉底 → 剩余 Credits 排序。
  - 后端在 `upsert_import_payload` 对新建或尚无 usage 的账号调度异步 `refresh_account_async`，并在成功落盘后发 `accounts:changed` 以触发前端刷新。
  - 全量刷新使用独立批处理 CLI（Cursor batch refresh）写盘；GUI 侧可用「刷新全部」触发同等逻辑。
- **涉及文件/模块**:
  - `src/pages/CursorAccountsPage.tsx`
  - `src/types/cursor.ts`
  - `src/locales/zh-CN.json`
  - `src-tauri/src/modules/cursor_account.rs`
  - `src-tauri/Cargo.toml`
- **验证方式**:
  - UIA 截图验证 WebView 正常加载（非 localhost 拒绝连接），并在搜索命中账号时看到「待查询配额」角标与占位文案。
  - 运行全量刷新后，磁盘统计 `pending=0`，并验证典型账号（如 `bh0hu18g@gmail.com`）在刷新后写入 `quota_query_last_error` 时 UI 显示失败角标。
- **风险或注意事项**:
  - 批处理 CLI 写盘不会自动推前端事件；需要 UI 重载列表（切页/重开）以读到最新磁盘状态。

### 2026-06-25

- **触碰功能**: Cursor 本地换号 → Cockpit 列表/current 长驻同步。
- **用户目标是否变化**: 否；用户实测 6/24 与重启后均可用，长驻会话会脱节（搜不到新导入号、current 不更新）。
- **本次目的**: 后端 `accounts:changed`（TokenKeeper 导入/换 current、导入后配额刷新）到达时，Provider 账号页自动 `fetchAccounts()`，避免仅 mount 拉一次列表。
- **实现手段**: `useProviderAccountsPage` 监听 `accounts:changed`（同 platform、`reason !== delete`）→ `fetchAccounts()`（内含 `fetchCurrentAccountId`）。
- **涉及文件/模块**: `src/hooks/useProviderAccountsPage.ts`
- **验证方式**: 长驻 Cockpit 不重启；Cursor 换到库外新号后 ≤20s 内 ALL 计数与磁盘一致，搜索邮箱片段可命中，current 标记更新。
- **风险或注意事项**: 不对 `delete` reason 重复拉列表；未恢复 `7761f453` 配额写盘行为。

### 2026-06-25

- **触碰功能**: F-002 多开自动选号 + 实例页配额展示。
- **用户目标是否变化**: 否；用户要求多开与总览同一套 pick，并解释实例 36 误选失效号。
- **本次目的**:
  - 定位 `9bc33ccd` 将多开 pick 从 `pick_cursor_rotation_account` 回滚为 `pick_account_for_auto_switch` 的回归；工作区未提交补丁已恢复总览对齐 pick。
  - `probe_cursor_account_live_auth` 须同时验证 usage API（仅 user meta 会在配额端已失效时仍放行）。
  - 多开实例行配额预览有 `quota_query_last_error` 时与总览一致显示失败，不得仍用缓存 0%。
- **实现手段**:
  - 保留 `cursor_instance.rs` 中 `pick_cursor_rotation_account` + `ensure_cursor_overview_pickable` + `refresh_and_ensure_overview_pickable` 链。
  - `probe_cursor_account_live_auth` 并行拉 user meta + usage summary，任一非 transient 失败则写 `quota_query_last_error` 并拒绝。
  - `buildCursorAccountPresentation` 遇 `quota_query_last_error` 返回失败文案 quota item。
- **涉及文件/模块**: `src-tauri/src/modules/cursor_account.rs`、`src/presentation/platformAccountPresentation.ts`、`src-tauri/src/commands/cursor_instance.rs`（未提交对齐补丁）。
- **验证方式**: audit `switch_trace_id=db641cae…` 全链路可读；编译通过；重打包后实例 36 Play 不得再选已标会话过期的号。
- **风险或注意事项**: 总览「当前」= 默认 profile 同步账号，与多开实例绑定账号本就可以不同，不是同一字段。

### 2026-06-25（恢复落地）

- **触碰功能**: F-002 多开自动选号（恢复 `73354e1a`）。
- **用户目标是否变化**: 否；用户明确要求把 `9bc33ccd` 回滚的多开 pick 改回来并编译 UI 验收。
- **本次目的**: 正式恢复 `pick_cursor_rotation_account` + `ensure_cursor_overview_pickable` + `refresh_and_ensure_overview_pickable` + 手动 Play 同样走 `ensure`；撤销 `9bc33ccd`（2026-06-22）对 `cursor_instance.rs` 的旧 pick 回退。
- **实现手段**: 工作区 `cursor_instance.rs` 对齐 `73354e1a`/`db749df9`；`probe` 双 API；实例配额展示读 `quota_query_last_error`。
- **涉及文件/模块**: `src-tauri/src/commands/cursor_instance.rs`、`src-tauri/src/modules/cursor_account.rs`、`src/presentation/platformAccountPresentation.ts`。
- **验证方式**: `npm run tauri build` → 覆盖 `%LocalAppData%\Cockpit Tools\cockpit-tools.exe` → UIA 打开 Cursor 多开实例页截图验收。
- **回归锚点**: 好版本 **`73354e1a`（2026-06-19）**；弄没 **`9bc33ccd`（2026-06-22 09:08）**。

### 2026-06-25（多开 Log in 页）

- **触碰功能**: F-002 多开 inject + 切号验收。
- **现象**: 实例 36（`d8a1817c…`）22:16 Play 绑定 `zines.welds_3y@icloud.com`；audit 全绿但 Cursor 为 Log in；实例 profile `vscdb` 启动后 `accessToken`/`refreshToken` 被清空。
- **根因**: 多开链未打 `main.js` machineId patch（仅 broken 修复）；post-audit 只验 HTTP API，未验 profile `vscdb` 落盘是否被 IDE 抹掉。
- **修复**: 多开 switch 调用 `patch_cursor_machine_id`；`spawn_switch_post_audit` 启动 4s 后 `verify_switch_tokens_persisted_in_profile`，失败记 `vscdb_fail` 且不算「切号验收通过」；inject 写盘用 `resolve_vscdb_auth_tokens`。
- **涉及文件**: `cursor_switch_align.rs`、`cursor_account.rs`、`cursor_instance.rs`。

### 2026-06-25（多开可用性对齐总览）

- **触碰功能**: F-002 多开自动选号 + 切号前可用性。
- **用户纠正**: 根因不是「挑额度最高」；是 HEAD 去掉总览同源可用性检查；禁止恢复多开 `sync_sidebar`（毁对话）。
- **修复**:
  - 多开 auto pick 恢复 `pick_cursor_rotation_account` + 选后 `refresh_and_ensure_overview_pickable`（未查到额度 / 待查询 = 不可用）+ probe 重试。
  - 总览指定账号 Play 同样走 refresh + ensure。
  - 多开 `switch_cursor_account_to_profile` 不含侧栏同步。
- **构建**: `target/release/cockpit-tools.exe` → `%LocalAppData%\Cockpit Tools\cockpit-tools.exe` SHA `68853BD3…`（NSIS  bundler 网络失败，release exe 可用）。
- **UI**: MCP 证明 Cockpit 非空白、Cursor 账号页、「多开实例」Tab 可定位；实例 Play / Cursor 登录待本机再点验。

### 2026-06-25（剔除侧栏同步死代码）

- **触碰功能**: F-002 多开 profile 隔离。
- **变更**: 从 `cursor_account.rs` 删除 `SIDEBAR_SYNC_KEY_PREFIXES` 与 `sync_sidebar_state_from_default_to_profile`（已无调用；该逻辑会用默认 profile 的 composer/glass/chat 键覆盖多开 vscdb，毁对话记录）。

### 2026-06-25（本地当前账号自动入库与 current 同步）

- **触碰功能**: Cursor 本地换号 → Cockpit 列表/current 长驻同步（**F-010** / TokenKeeper）。
- **用户目标是否变化**: 否；HR-20260625-001/003/004 要求默认 profile 换号后自动入库、标 current、列表可搜。
- **根因**: `sync_local_cursor_from_default_profile` 被写成空 stub（为修编译占位），`provider_token_keeper` 每 20s 调用却无读盘/写库/更新 `provider_current_accounts.json`。
- **本次目的**: 恢复 TokenKeeper 链：读 `state.vscdb` → 必要时 `upsert` 入库 → `set_current_account_id("cursor")` → `accounts:changed`（`local-auto-import` / `local-current-sync`）。
- **实现手段**:
  - `find_account_for_import_payload` + `local_import_payload_matches_account`：无变更时跳过写盘（避免 20s 全量 upsert）。
  - 有变更时 `upsert_account_with_outcome` + `record_import_backup`；current 与本地 profile 账号 id 不一致时更新 current。
- **涉及文件/模块**: `src-tauri/src/modules/cursor_account.rs`（`sync_local_cursor_from_default_profile`）；消费方 `provider_token_keeper.rs` 不变。
- **验证方式**: 默认 Cursor 登录 `spica_vision4w@icloud.com` 时，≤20s 内磁盘 `cursor_accounts.json` 可搜到、 `provider_current_accounts.json` cursor 指向该号；长驻 Cockpit UI 经 `accounts:changed` 刷新。
- **风险或注意事项**: 不恢复侧栏同步；多开实例 bind/current 仍可与总览 current 不同。

### 2026-06-26（部署链回归 + 验收失职纠正）

- **触碰功能**: F-005 GUI 验收、F-010 本地同步、F-011 部署链。
- **用户纠正**: 做完须检查做了什么；不得用磁盘/脚本结果代替 UI；「你在幻想什么」= 宣称修好但用户窗口仍是 WebView 网络错误。
- **根因**:
  1. `sync_local_cursor_from_default_profile` 曾为空 stub（已在 2026-06-25 修复）。
  2. 交付时仅复制 `cargo build --release` 产物（SHA `300C6278…`），未走 `npm run tauri build` 完整链 → WebView 显示 `localhost - 网络错误`，用户看不到账号页。
  3. Agent 用磁盘 current 一致宣称「已修好」，违反 `post-build-ui-must-verify.mdc` 与 HR-20260625-009。
- **处理**:
  1. 执行 `npm run tauri build`（NSIS 因缺私钥 exit 1，但 `target/release/cockpit-tools.exe` 已生成）。
  2. 覆盖安装 SHA **`73A739EC…`**；MCP 证实 WebView 为 `Tauri + React + Typescript - Web 内容`（非网络错误）。
  3. 磁盘：Cursor 本地与 Cockpit current 均为 `WitaszekGadd56@outlook.com`（2026-06-26 复测）。
- **涉及文件**: `cursor_account.rs`（sync 逻辑）、部署流程、`.cursor/user-history-requirements.md`、本档案 F-010/F-011。
- **验证方式**: 部署后必做「SHA → 启进程 → MCP WebView 非网络错误 → 相关页」清单；后端与 UI 分开标注。
- **档案缺口（本次补）**: 原 F-001～F-009 未单独记录 F-010 本地同步、F-011 部署链；HR-002 待查询配额/新导入立即刷新等待独立 F 条目（仍仅在历史要求与增量记录，未升格为 F-012）。

### 2026-06-26（修好/没修好二态规则）

- **触碰功能**: F-005 GUI 验收、沟通结论。
- **用户要求**: 不发明「按什么标准说已修好」的分拆话术；已修好=已修好，没修好=没修好，撒谎=撒谎。
- **规则**: `.cursor/rules/honest-fix-status-only.mdc`（`alwaysApply`）；同步修订 `post-build-ui-must-verify.mdc`、`chinese-response-style.mdc`；Codex 镜像 `.codex/rules/honest-fix-status-only.md`；历史 **HR-20260626-002**。

### 2026-07-16（Cursor 配额最旧优先刷新 · v1.3.6）

- **触碰功能**: Cursor 配额自动刷新（非 F-010 跟号；跟号仍只对齐 current）。
- **用户目标是否变化**: 否；用户确认「全量每轮从头扫 → 大量账号额度陈旧」需要新版本修。
- **约束**: 仍遵守 HR-20260626-005——**不恢复** `cursor_refresh_scheduler` / 并发 batch；保持串行。
- **本次目的**: 自动刷新不再每轮按索引从头扫完全库；优先刷 `usage_updated_at` 最旧的账号，每轮有数量与时限，下一轮继续挑最旧。
- **实现手段**:
  - `refresh_tokens_stale_first(max_count, max_duration)`：按调度键升序串行；失败/刚尝试过用进程内 attempt map + `quota_query_last_error_at` 冷却。
  - 自动刷新每轮 `max_count=120`、墙钟 8 分钟；手动全量 `max_count=None` 仍扫全部（同样最旧优先）。
- **涉及文件**: `cursor_account.rs`、`commands/cursor.rs`、`useAutoRefresh.ts`、`cursorService.ts`、`cursor_refresh_batch` bin。
- **验证方式**: 部署后看日志含「最旧优先刷新开始」；陈旧号 `usage_updated_at` 在多轮自动刷新后应前进；UI 非网络错误。
- **风险**: 单号 transient 失败仍不写新 usage（既有 F-004）；冷却后会再试。


- **触碰功能**: F-007、F-008、F-011。
- **用户目标是否变化**: 否；纠正为必须同时真用 fork tip 与线上 tip，角标是露馅不是单修项。
- **本次目的**: 作废拼装 `1A0EC65E…`/`96ea7855`；从 `918980e9` 真合并 `da0deca4`。
- **实现手段**: 分支 `sync-upstream-v1.3.0-20260714-redo`；merge commit `27c3ee93`（parents `918980e9`+`da0deca4`）；Cursor 页/模块=fork blob（~1379 行）；Grok/Zcode=上游 blob；禁整棵换 `src/`。
- **验证方式**: 源码 hash 对照；无「Take upstream frontend」类提交。
- **风险**：初写「确认前不标能用」已被 **HR-20260715-006** 推翻；验收由 Agent 证据闭环，达标即标能用/已修好。
- **增量（2026-07-15）**: 注册表将 SHA `DEA43F62…` 升为**能用**；UIA 再验 Cursor 页达标。

### 2026-07-15（upstream v1.3.2 正式版同步）

- **触碰功能**: F-006、F-007、F-008；Cursor 常驻跟号（`sync_cursor_local_watch` / `accounts:changed`）。
- **用户目标是否变化**: 否。
- **本次目的**: 主仓正式发布 **v1.3.2**（`a84a97cb`）高于已记 1.3.0，执行隔离合并与交付；合并前恢复并保留 fork 常驻 watch。
- **实现手段**: 分支 `sync-upstream-v1.3.2-20260715`；merge `a6efd371` parents=`ebd0dca8`+`a84a97cb`；strip workflows base `upstream-v1.3.2-base`；release exe SHA `3374C285…` + NSIS；PR #7。
- **涉及文件/模块**: `provider_token_keeper.rs`、`useProviderAccountsPage.ts`、`cursor_account.rs`（secure load）、安装/Release/注册表/SCOPE/AGENTS。
- **验证方式**: ProductVersion 1.3.2；UIA Cursor `ALL (1873)`/`配额未查询`/`FREE`；Edge UIA 锁 PR#7/Release/分支页；源码调用点含 `sync_cursor_local_watch`。
- **风险**: updater 签名缺私钥导致 `tauri build` exit≠0，但 exe/NSIS 已产出；Play/多开 GUI 点验仍待单独立项。

### 2026-07-18（upstream v1.3.8 正式版同步 · 交付 v1.3.13）

- **触碰功能**: F-006、F-007、F-008；Cursor 常驻跟号（F-010）；配额排序/最旧优先刷新（fork tip 1.3.12 能力保留）。
- **用户目标是否变化**: 否。
- **本次目的**: 主仓正式发布 **v1.3.8**（`fb291416`）高于文档已记集成 **1.3.2**；在隔离分支合入并完成构建/UIA/个人仓 release/网页复核。
- **实现手段**:
  - 起点：fork tip `fork-20260717-quota-sort-refresh`（v1.3.12）+ 合并前 baseline draft release。
  - 分支 `sync-upstream-v1.3.8-20260718` @ `1eaa1bec`；merge `22e8f6b4`（upstream `fb291416`）→ ProductVersion **1.3.13**。
  - 对照 PR #11（base=`upstream-v1.3.8-base` strip workflows，head=同步分支）。
  - Release `sync-upstream-v1.3.8-20260718` 资产 `cockpit-tools.exe` SHA **`646020F6…`**。
- **涉及文件/模块**: 合并后树；`provider_token_keeper.rs` / `useProviderAccountsPage.ts` 调用点保留；规则/SCOPE/AGENTS/本档案最新构建记录。
- **验证方式**: 安装 exe ProductVersion 1.3.13 + SHA `646020F6…`；PrintWindow 标题 `Cockpit Tools`、Cursor 页 **`ALL (2119)`**（磁盘 2119）；Edge UIA：分支 tab / PR#11 tab / Release 页 `cockpit-tools.exe` + digest `646020f6…`。
- **风险**: WiX/`light.exe` bundle 失败，交付为 release exe 直拷（无 NSIS）；Play/多开 GUI 点验仍待单独立项；PR base 为 strip 的 `upstream-v1.3.8-base`（个人仓对照惯例），非 `origin/main` 旧 tip。

### 2026-07-19（upstream v1.3.10 正式版同步 · 交付 v1.3.14）

- **触碰功能**: F-006、F-007、F-008；Cursor 常驻跟号（F-010）；配额排序/最旧优先刷新（fork tip 1.3.13 能力保留）。
- **用户目标是否变化**: 否。
- **本次目的**: 主仓正式发布 **v1.3.10**（含 v1.3.9；tip `b331b093`）高于文档已记集成 **v1.3.8**；隔离合并并完成构建/UIA/个人仓 release/网页复核。
- **实现手段**:
  - 起点：`sync-upstream-v1.3.8-20260718` @ `b2d3d3aa`（合并前代码+exe 已在个人仓）。
  - 分支 `sync-upstream-v1.3.10-20260719`；merge `5d15a88f` parents=`b2d3d3aa`+`b331b093` → ProductVersion **1.3.14**；i18n `pendingQuery` 对齐 preflight。
  - 对照 PR #12（base=`upstream-v1.3.10-base` strip workflows，head=同步分支）。
  - Release `sync-upstream-v1.3.10-20260719` 资产 `cockpit-tools.exe` SHA **`65A51E85…`**。
- **涉及文件/模块**: 合并后树（Codex/Trae/cliproxy 上游变更）；`provider_token_keeper.rs` / `useProviderAccountsPage.ts` 调用点保留；规则/SCOPE/AGENTS/本档案最新构建记录。
- **验证方式**: 安装 exe ProductVersion 1.3.14 + SHA `65A51E85…`；PrintWindow 标题 `Cockpit Tools`、Cursor 页 **`ALL (2129)`**（磁盘 2129）；Edge UIA：分支 / PR#12 / Release 页 exe。
- **风险**: WiX/`light.exe` bundle 失败，交付为 release exe 直拷；Play/多开 GUI 点验仍待单独立项。

### 2026-08-02：upstream v1.3.15 正式版同步 → 交付 1.3.17

- **上游 tag**：`v1.3.15` @ `939d5d72`
- **同步分支**：`sync-upstream-v1.3.15-20260802` @ `c744cc6b`
- **预合并 tip**：本地 `sync-upstream-v1.3.14-20260725`（交付 1.3.16，此前未 push）
- **Release**：`sync-upstream-v1.3.15-20260802`；exe SHA-256 `42814DE79D929B88CFE4E6FF2852BE86B19A7E41B52F79C03198B63DEA166607`；ProductVersion **1.3.17**
- **已记集成基线**：upstream **v1.3.15** / 交付 **1.3.17**

