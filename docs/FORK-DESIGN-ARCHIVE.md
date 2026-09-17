# Cockpit Tools Fork Persistent Design Archive

**最后更新**: 2026-08-24  
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

**代码设计**: 定时任务检查 upstream latest release、最新 tag 和版本字段；只有版本高于已记录集成版本才进入合并流程。合并起点使用本地最新可用分支（当前默认 `sync-upstream-v1.3.16-20260804` / 已记 **upstream v1.3.16 → 交付 ProductVersion 1.3.18**；上一项正式同步为 v1.3.15→1.3.17），保留 fork 功能（含 `sync_cursor_local_watch`）和 release 边界。

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

### F-012 多开实例无感换号

**用户目的**: 多开 Cursor 换号时不关窗、热写登录态；默认 Cursor 主实例不得被关掉。换号后对话仍可用，侧栏底部邮箱必须变成新号。

**自然语言设计**: 多开换号应像续杯/虚备那样热写认证态，不是关窗再开。侧栏必须还能点开对话。底部邮箱以界面真实登录态为准，不得靠反复改页面文字伪装。验收必须走总控里点多开启动，不能只用命令行顶替。

**代码设计**:
- 多开热写：实例目录投递无感态 → 热写该实例库 → 单次把认证态推进运行中窗口的存储；后台只做只读校验，不再反复全量灌脚本。
- 禁止再：递增强制令牌计数、定时直改侧栏底部邮箱文字、弹成功条、软重载页面。旧做法会打坏侧栏，点不动。
- 总控多开启动走应用内点击：先切到 Cursor 多开页，再点实例启动。总控若收到托盘、主窗标题不是「Cockpit Tools」，点击发不到真实界面，须先唤回主窗。
- 多开 exe 与实例目录与默认 Cursor 隔离；默认实例切号仍遵守 F-001。
- 运行中自动换号只扫多开实例，禁止对默认实例热写。
- 续杯 util/EH：只写入多开安装树 `main.js` / 共享进程 / ExtHost 头钩，令牌文件落在该实例 `cockpit-seamless/`；默认安装禁止写入。

**保留边界**: 不得为换号杀掉默认 Cursor；不得恢复侧栏文字补丁；不得删除无忧式传统切号路径。

**验证方式**: 总控主窗标题为 Cockpit Tools 后点多开启动；多开窗底部邮箱与磁盘邮箱一致；侧栏能点开已有对话并得到回复；默认 Cursor 进程仍在。

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

### 2026-08-04（upstream v1.3.16 正式版同步 · 交付 v1.3.18）

- **对照 PR**: https://github.com/aliceemoce/cockpit-tools/pull/15

- **触碰功能**: F-006、F-007、F-008、F-010、F-011；Cursor watch / 轮换调用点保留。
- **用户目标是否变化**: 否。
- **本次目的**: 主仓正式发布 **v1.3.16**（`e1ef55ce`）高于已记集成 **v1.3.15**；隔离合并并完成构建/UIA/个人仓 release/网页复核。
- **实现手段**:
  - 合并前：`origin/sync-upstream-v1.3.15-20260802` @ `612fe2db` + Release exe SHA `42814DE7…` 已在个人仓。
  - 分支 `sync-upstream-v1.3.16-20260804`；merge `94e4bb99` parents=`612fe2db`+`e1ef55ce` → ProductVersion **1.3.18**。
  - 对照 PR #15：base=`upstream-v1.3.16-base`（upstream tip strip workflows；OAuth 无法直接推含 workflow 的 `upstream/main` 镜像），head=同步分支。
  - Release `sync-upstream-v1.3.16-20260804` 资产 `cockpit-tools-1.3.18.exe` SHA **`67E971E2…`**。
- **涉及文件/模块**: Codex/cliproxy/Windows NSIS 快捷方式等上游变更；`provider_token_keeper.rs` / `useProviderAccountsPage.ts` / `pick_cursor_rotation_account` 调用点保留；规则/SCOPE/AGENTS/本档案最新构建记录。
- **验证方式**: 安装 exe ProductVersion 1.3.18 + SHA `67E971E2…`；PrintWindow 标题 `Cockpit Tools`、Cursor 页 **`ALL (2604)`**（磁盘 2604）；系统浏览器+UIA 复核分支 / PR / Release 资产。
- **风险**: Tauri updater 签名缺私钥（exit 1）但 MSI/NSIS/exe 已产出，交付为 release exe 直拷；Play/多开 GUI 点验仍待单独立项；本机 UIA MCP catalog 空时用 `uiautomation`+PrintWindow（已锁定 hwnd）。

### 2026-08-24（多开无感换号 + 总控启动验收）

- **触碰功能**: F-001、F-002、F-012。
- **用户目标是否变化**: 否；当场要求多开无感换号、不动默认 Cursor、换号后对话可用、底部邮箱变成新号；并问多开侧栏完全无法使用。
- **本次目的**: 修好多开换号后侧栏点不动；用总控界面启动多开，核对邮箱、侧栏、对话。
- **实现手段**:
  - 去掉多开换号脚本里对侧栏底部邮箱的定时改字、强制计数、成功条和软重载；后台改为最多几轮只读校验。
  - 总控启动须先有标题为 Cockpit Tools 的主窗；托盘唤起壳窗点不到启动。唤回主窗后应用内点击启动实例成功。
- **涉及文件/模块**: `src-tauri/src/modules/cursor_account.rs`、`src-tauri/src/modules/gui_in_app_click.rs`、`scripts/verify_dual_cursor_instances.py`、`scripts/_evidence_multi_seamless/`。
- **验证方式**:
  - CLI 无感切号：Robin / Sotero 底部邮箱对齐，侧栏可点，默认 Cursor 仍在。
  - 总控 Play：前端已点击多开启动；实例起来；底部邮箱 `jzvj0743@outlook.com`；侧栏点开「Multi V3 cockpit strict reply」；回复 `GUI_PLAY_V1`；默认 Cursor 未关。
- **风险或注意事项**: 总控收到托盘时 deep link 点击会发到空界面；须先唤回主窗。对话验收须在已打开的对话页，不能停在 Home / Plan New Idea。

### 2026-08-24（运行中自动换号）

- **触碰功能**: F-012；运行中自动换号。
- **用户目标是否变化**: 否；当场要求先上传当前版本，再做成自动换号；禁止用更短路径偷懒；自动换号验收前不开工续杯完整注入复刻。
- **本次目的**: 多开实例在运行中额度到阈值或对话限流时自动无感换号；不抢默认窗。
- **实现手段**:
  - 读本机配置开关与阈值，不再写死关闭。
  - 启动后每二十秒扫已跑多开实例；冷却六十秒；走现有多开无感切号。
  - 总览工具条增加运行中开关；快捷设置里原有开关仍写同一配置。
  - 本地跟号不再对默认实例跑假自动换号。
- **涉及文件/模块**: `src-tauri/src/modules/cursor_account.rs`、`src-tauri/src/commands/cursor_instance.rs`、`src-tauri/src/lib.rs`、`src/pages/CursorAccountsPage.tsx`。
- **验证方式**: 覆盖安装后打开运行中开关；多开绑定号到阈值后日志出现自动换号成功；多开窗邮箱变新号且可聊；默认 Cursor 仍在；关掉开关后不再换。
- **风险或注意事项**: 完整续杯注入复刻已在自动换号验收后开工。

### 2026-08-27（禁止抢续杯无感通道 · 范围仅限默认切号）

- **触碰功能**: F-001 默认切号；与续杯管家共存边界。
- **用户目标是否变化**: 是；当场纠正「总之不能抢别人的」——**默认 Play/默认无感**不得占用续杯管家无感通道。
- **本次目的**: 默认切号只写本机默认 Cursor 库，不再写 `.wuxian-assistant` / `wx_*`、不再关对方自动换号、不再后台对抗盖写。
- **实现手段**: 删除 `apply_xubei_seamless_hot_path` 及粘号/续盖；`default_seamless_switch_steps` 仅 `switch_tokens_in_profile_db_live` + `upsert_storage_json_ids`；`read_wuxian_get_token_email` 保留只读对照。
- **涉及文件/模块**: `src-tauri/src/modules/cursor_account.rs`、`.cursor/workspace-plan.md`。
- **验证方式**: `cargo check` 通过；源码无 `wx_token` / `apply_xubei` 写入点（**续杯专用入口恢复后除外**，见 2026-09-02）。
- **风险或注意事项**: 若续杯注入仍在默认 Cursor 轮询 get-token，总控只写库后侧栏可能仍被续杯盖回——这是**默认链**隔离后果；**不**适用于续杯专用链。

### 2026-09-02（续杯链必须与管家同通道 · 总控内自成一体）

- **触碰功能**: F-012 续杯拉号+无感换号；F-001 默认切号边界澄清。
- **用户目标是否变化**: 是；用户纠正：续杯无感**不能**做成「半套依赖管家开着」；**必须**与续杯管家**同一通道**；「不抢」**只约束默认**；无忧/续杯助手/续杯管家三程序换号逻辑**多半矛盾**，须分链。
- **用户原话**: 「这个不能不抢管家通道，必须和管家是一个通道，写在规划里，不抢的是默认，三种换号软件的逻辑多半是矛盾的」
- **本次目的**:
  - **续杯专用入口**（`cursor-xubei-pull`、`cursor-xubei-seamless-switch`）：写 `.wuxian-assistant` / `wx_*`、对齐 get-token 热换、对齐管家 `apply_account` 语义；目标态为**总控 alone 可用**（登录、注入、本地服务均在总控内）
  - **默认 Play/闪电/自动换号**：仍只写默认 Cursor 本库，**不写** wuxian（2026-08-27 边界保留）
- **实现手段（目标态，待完成）**:
  - 总控内续杯云端登录与 device_code（替代只读 `~/.cursor-switch-assistant/auth.json`）
  - 总控内 Cursor main.js 注入/还原（复刻管家补丁，非 Play 路径跳过）
  - 总控内本地 get-token HTTP 服务（端口与协议对齐管家）
  - 续杯无感 UI 对齐管家：注入/还原、激活无感、自动换号、重置机器码、换号后发「继续」
  - 已恢复（2026-09-02 半链）：`apply_xubei_seamless_hot_path` / `xubei_seamless_switch_account` / `pull_and_xubei_seamless_switch` — **仍依赖管家进程**，属过渡态
- **涉及文件/模块**: `xubei_switch_client.rs`、`cursor_account.rs`、待增注入/get-token 模块、`CursorAccountsPage.tsx`、`program-plan.mdc`、`.cursor/workspace-plan.md`。
- **验证方式**: 关闭续杯管家进程；总控完成拉号+无感换号；get-token 返回新邮箱；侧栏变号；默认 Play 仍不写 wuxian。
- **风险或注意事项**: 三程序逻辑互斥——默认链与续杯链**禁止混用**；同一时刻只应有一条链写当前 Cursor；续杯链与管家同通道意味着磁盘态与协议须对齐，不能各写各的格式。

### 2026-09-03（续杯管家池内换号接线 + 四开关写盘）

- **触碰功能**: F-012 续杯拉号+无感换号；续费控制台管家面板。
- **用户目标是否变化**: 否；用户要求继续重做管家一整块，禁止占位壳。
- **本次目的**:
  - 续费台「换号」走池内 wuxian 链（`switch_cursor_account_from_xubei_pool`），不得误接默认 Play
  - 四开关真实读写 `~/.wuxian-assistant` 与 `~/.cursor-switch-assistant/config.json`
- **实现手段**:
  - `xubei_renewal_prefs.rs` + 命令 `get_xubei_renewal_prefs` / `set_xubei_renewal_pref`
  - `renewal_console_status` 增加 `autoResetMachine` / `autoSendContinue`
  - `CursorRenewalConsole.tsx` 开关去掉 `disabled`，拨动即写盘
  - 四开关加 `data-action-id`（`cursor-renewal-xubei-toggle-*`），走应用内点击 / `cockpit-tools://click/…`
- **验证方式**（2026-09-03 已验收）:
  - 单元测试 `write_auto_reset_machine_roundtrip` 通过
  - 覆盖安装 debug exe SHA `1FD4D6F9…`；日志 `前端已点` 四开关 + `续杯偏好写入` 键值成对出现（10:53 / 10:55 / 10:57）
  - 磁盘对照：拨 `seamless`/`autoSwitch`/`autoResetMachine`/`autoSendContinue` 后 `seamless_state.json`、`auto_switch_pref.json`、`auto_resume_continue_pref.json`、`config.json` 字段随之变
  - PrintWindow 留证 `_verify_renewal_prefs_final.png`（标题 `Cockpit Tools`）
- **风险或注意事项**: 自动换号状态机、注入/还原、get-token 内嵌仍未完成；开关写盘 ≠ 自动换号已跑通；深链若连开多实例会产生无标题僵尸窗，验收前须只留一个有标题主进程。

### 2026-09-06（续费台真机拉号/无感/四开关 + deep link 点击修复）

- **触碰功能**: F-012 续杯拉号+无感换号；F-014 应用内点击 / DeepLink。
- **用户目标是否变化**: 否；用户多次「继续」要求真机验收拉号、无感换号、四开关写盘。
- **本次目的**: 消灭续费台 deep link 点击 `timeout_no_ack`；真机点通拉号/无感/四开关并留下磁盘与 get-token 证据。
- **实现手段**:
  - `deep_link_actions.rs`：`handle_click` 改为 `spawn_blocking` 等 ack，避免堵死主线程导致 WebView 无法回执
  - `handle_ui_state` 去掉全量 `list_accounts`，只读索引条数 + 点击 ack，避免轮询卡死
  - `gui_in_app_click` 续费/nav 超时 45s；前端续费/nav 长重试
- **验证方式**（已验收）:
  - 覆盖安装 SHA 前缀 `974BBE3D…`
  - `_verify_nav_keepalive/PULL_SEAMLESS_v3_report.json`：`pull_clicked` / `seamless_email_changed` / `prefs_changed` 均为 true；池新增 `EckelWalner47@outlook.com`；无感后 get-token=`BucholzLeonelli361@outlook.com`
  - 验收拨关后已应用内点击恢复四开全开
- **风险或注意事项**: 总控内嵌 wuxian 服务/注入/还原仍未完成（`workspace-plan` 2026-09-03 未勾项）；功能齐全与管家同级代码量仍未宣称。

### 2026-08-28（续杯 main.js 补丁共存 · Play 不抹机器码）

- **触碰功能**: F-001 默认切号；与续杯管家「重置机器码」共存。
- **用户目标是否变化**: 否；续做「续杯管家重置机器码失灵」——Cockpit 不得覆盖续杯/虚备已打的 `main.js`，Play 切号不得反复重写 `storage.json` / `machineId` 抵消续杯重置。
- **本次目的**: 检测第三方续杯补丁标记后，Cockpit 跳过自有 csp patch、禁止从 `.cursor-backups` 恢复原版抹掉续杯补丁；默认无忧切号路径在第三方补丁存在时跳过 Gh/Jh 指纹重置。
- **实现手段**:
  - `crates/cockpit-core/src/modules/patcher/cursor_patch.rs`：`THIRD_PARTY_MAIN_JS_MARKERS`（`global.__cs_mid`、`MOCURSO`、`/*i0*/` 等）；`patch_cursor_main_js` / `restore_cursor_main_js` 遇第三方标记返回 false。
  - `src-tauri/src/modules/cursor_switch_align.rs`：`restore_main_js_from_backup` / `patch_cursor_machine_id` 第三方 guard；`cursor_main_js_has_third_party_renewal_patch` 供切号链查询。
  - `src-tauri/src/modules/cursor_account.rs`：`nirvana_traditional_switch_steps` 在 `main.js` 含续杯补丁时跳过 `reset_storage_json_ids_for_profile` 与 `reset_machine_id_file_for_profile`；**`default_seamless_switch_steps`（Play/默认无感主路径）** 在同样条件下跳过 `upsert_storage_json_ids`。
- **涉及文件/模块**: 上列三文件；`patcher/mod.rs` 导出；单元测试 `cursor_patch` 三例。
- **验证方式**: `cargo test -p cockpit-core cursor_patch` 3/3；本机 `C:\Program Files\Cursor\resources\app\out\main.js` 含 `__cs_mid`×8、无 `csp1`；覆盖安装后 Play 切号日志须见「跳过 storage.json 与 machineId 文件重置」；续杯「重置机器码」后额度条仍可见（UI 验收待做）。
- **风险或注意事项**: 多开实例切号走 `switch_tokens_in_profile_db_live` 本就不写 Gh/Jh，无需重复 guard；关闭 Cockpit 不自动恢复磁盘改动；额度仍失败时需查代理/API/续杯 get-token 与写库冲突（与 main.js 互踩正交）。

### F-014 应用内操作与 DeepLink 系统级触发 (In-App Actions)

**用户目的**: 解决外部脚本模拟点击（鼠标坐标、UIA 外部驱动）不稳定、易受窗口焦点干扰的问题，实现应用内部精准可控的按钮触发。**2026-09-04**：消灭「派发即成功」的虚假能力与幻想完成；卡住须有可读原因。

**自然语言设计**: Agent、CLI 或 DeepLink 触发 UI 操作时，不走外部假鼠标点击，而是由后端命令或系统协议直接唤醒应用内部事件总线，精准触发前端对应按钮的原生 DOM 点击。**成功仅当前端找到可点控件并完成 click 且回执成功**；超时、找不到、禁用、去重跳过均须失败并带原因码。

**代码设计**:
- 后端：`src-tauri/src/modules/gui_in_app_click.rs` 的 `trigger_click_wait`；命令在 `src-tauri/src/commands/system.rs`（`gui_trigger_click` / `gui_click_ack` / `gui_click_recent_acks` / `gui_click_latest_ack`）。
- 派发 payload：`{ action_id, request_id }` → 事件 `gui:trigger-click`；前端 ack 按 `request_id` 唤醒等待方。
- 前端：`src/App.tsx` 按 `data-action-id` 查找；disabled / 未找到须 `success=false` 并带页提示。
- DeepLink：`cockpit-tools://click/<action_id>` 同样 wait-ack，失败写 warn 并 emit `deep-link-action-result`。

**保留边界**: 凡目标按钮已挂载 `data-action-id` 锚点，GUI 验收与自动化一律强制走应用内点击，严禁改用 UIA 或外部坐标点击。**派发成功 ≠ 已点击**。

**验证方式**: `gui_trigger_click` 返回 `success: true` 且含 `element_info`；假 action_id / 错页必须失败原因码，不得 Ok。

#### 增量 · 2026-09-04 wait-ack

- **触碰功能**: F-014。
- **用户目标**: 解决卡顿、幻想、虚假能力、卡死无知觉。
- **实现手段**: pending + recv_timeout；原因码 `timeout_no_ack` / `element_not_found` / `element_disabled` / `deduped_skipped`；注册 `gui_click_recent_acks`。
- **涉及文件**: `gui_in_app_click.rs`、`system.rs`、`lib.rs`、`App.tsx`、`deep_link_actions.rs`、规则 `cockpit-in-app-control-only.mdc` / `no-script-substitute-ui.mdc`。

---

### 2026-08-25（Nirvana-Proxy 方案 A 原生 Sidecar 深度联动）

- **触碰功能**: F-013 & F-014。
- **用户目标是否变化**: 明确要求拒绝两个互不相干的独立程序，采用方案 A（原生 Sidecar 伴生引擎 + Cockpit 深度总控双向联动）。
- **本次目的**: 彻底打通生命周期强绑定、账号池动态推送、IDE 自动引流与前端控制面板。
- **实现手段**:
  - `crates/cockpit-core/src/modules/proxy/sidecar.rs`: Win32 Job Object 内核级绑定 + `CREATE_NO_WINDOW` 后台静默伴生运行。
  - `crates/cockpit-core/src/modules/proxy/account_sync.rs`: 将 Cockpit 管理的所有账号动态推送至原生代理轮换池。
  - `src/pages/ProxyManagerPage.tsx`: 增加原生 Sidecar 伴生引擎管理看板与账号池注入交互。
- **涉及文件/模块**: `crates/cockpit-core/src/modules/proxy/{sidecar.rs, account_sync.rs, mod.rs}`, `src-tauri/src/commands/proxy.rs`, `src/pages/ProxyManagerPage.tsx`, `.codewiki/modules/nirvana_sidecar_architecture.md`。
- **验证方式**: `cargo check --workspace` 与 `npm run build` 全量通过（Exit Code 0）。

#### 增量 · 2026-09-06 切页卡死 / 续费无窗 / ALL 不回落

- **触碰功能**: 账号列表与主路由保活；续费台外置同步；F-014 验收路径（deep link / 应用内截图）。
- **用户目标**: 应用内快速切页不卡死；进续费台不弹可见 PowerShell；账号总览 ↔ 应用多开互切后 ALL 保持全量。
- **实现手段**: 停全表浏览器缓存；keep-alive 隐藏跳过重活；Cursor 列表分页去令牌；分片未完成禁止更小集合覆盖更大内存池；多开页取消挂载/定时整表重拉；续费同步脚本 Hidden + CREATE_NO_WINDOW。
- **涉及文件**: `createProviderAccountStore.ts`、`useProviderAccountsPage.ts`、`InstancesManager.tsx`、`App.tsx`、`renewal_apps_auto_update.rs`、切片 `docs/*/0003-account-cache-wrong-place-freeze.md`。
- **验证方式**: 覆盖安装后 deep link 总览↔多开，截图 `ALL (4072)` 不回落；进续费台 20s 内无新可见 PowerShell。
- **备注**: 多开实例「账号不存在」若绑定 id 在磁盘池中确无文件，属陈旧绑定，与分片盖池已区分。

#### 增量 · 2026-09-06 续费台切走卡死

- **触碰功能**: Cursor 页子 Tab 保活；续费同步命令。
- **用户目标**: 从续费控制台切出去不得卡死。
- **实现手段**: `visitedCursorTabs` 保活总览/多开/续费；`sync_renewal_apps_auto_update` 异步 spawn_blocking；续费台先状态后同步。
- **涉及文件**: `CursorAccountsPage.tsx`、`CursorRenewalConsole.tsx`、`commands/cursor.rs`。
- **验证方式**: `LEAVE_02` 续费台 → `LEAVE_07` 总览 `ALL (4076)`；进程 Responding；安装 SHA `CE6305CE…`。

#### 增量 · 2026-09-09 Cursor 分片水合卡约 200

- **触碰功能**: Cursor/提供商账号分片 list 水合与 keep-alive 切页。
- **用户目标**: 磁盘数千号时前端不得长期只有约 200 且列表区空白。
- **实现手段**: `accountsHydrationComplete`/`totalHint`；隐藏页不 cancel 分片；未完成或池短于 hint 时切回重拉；后台 loading 中切回不二次 fetch。
- **涉及文件**: `createProviderAccountStore.ts`、`useProviderAccountsPage.ts`、`CursorAccountsPage.tsx`、切片 `docs/*/0005-cursor-chunk-hydration-stuck-200.md`。
- **验证方式**: tsc/build 通过；覆盖安装后 deep link 总览 ALL≈磁盘量且列表可见（qa 真机项仍未执行）。

### 2026-09-06（无忧原生适配器 · ASAR 指纹探测 · 前端能力接入）

- **触碰功能**: 无忧小助手续费控制台面板；F-001 无忧传统切号保留。
- **用户目标是否变化**: 否；用户要求无忧按钮对应真实原程序能力，不做占位。
- **本次目的**:
  - 新增 `wuyou_native.rs` 无忧原生适配器：ASAR 指纹探测、能力判定、fail-closed 策略
  - 前端无忧面板接入真实状态（指纹验证、材料就绪、SHA-256）
  - 无忧按钮按后端能力禁用/启用，拉号/无感显示不可用原因
  - 注册 `wuyou_traditional_switch` 命令，前置检查指纹验证
- **实现手段**:
  - `wuyou_native.rs`：比较工作区样本与安装 ASAR SHA-256，不一致时 fail-closed
  - `renewal_console_status.rs`：`collect_wuyou_status` 从适配器读取真值
  - `commands/cursor.rs`：`get_wuyou_native_status` + `wuyou_traditional_switch`
  - `CursorRenewalConsole.tsx`：无忧面板显示探测状态，按钮按 `backendReady` / `cloudPullAvailable` / `seamlessAvailable` 禁用
  - `cursorService.ts`：`WuyouNativeStatus` 类型 + `getWuyouNativeStatus` + `wuyouTraditionalSwitch`
- **涉及文件/模块**: `wuyou_native.rs`、`mod.rs`、`renewal_console_status.rs`、`commands/cursor.rs`、`lib.rs`、`cursorService.ts`、`CursorRenewalConsole.tsx`、`docs/adr/0001-wuyou-native-adapter-boundary.md`
- **ASAR 取证结论**:
  - `cursor:cloud-pull` 固定返回"即将开放" → `cloud_pull_available = false`
  - `cursor:seamless:ping` 缺 bridge 服务端 → `seamless_available = false`
  - `cursor:accounts:switch` 传统切号 → 已对齐 `cursor_switch_align.rs`
  - 工作区 ASAR 与安装 ASAR SHA-256 不同 → 当前 `fingerprint_verified = false` → 所有按钮禁用
- **验证方式**: `cargo check` ✓、`tsc --noEmit` ✓、`npm run build` ✓、单元测试 `uses_a_distinct_account_source_tag` ✓；完整 Tauri 构建进行中
- **风险或注意事项**: 三程序换号链互相独立，无忧模块不写管家目录/端口；指纹不一致时所有动作 fail-closed；自动更新后须重新取证


### 2026-09-10（纠正：运行中自动换号只服务默认页）

- **触碰功能**: F-012 运行中自动换号。
- **用户目标是否变化**: 是；用户明确纠正：不存在多开页自动换号，自动换号只存在于默认页面。
- **纠正对象**: 2026-08-24「只扫多开、禁止碰默认」实现与档案条目——一开始就接反。
- **本次目的**: tick 只监控默认 Cursor；多开不参与；阈值判定前刷新绑定号额度。
- **实现手段**: 重写 `tick_runtime_auto_switch`：读 default_settings 绑定与默认 userDataDir 进程；调用 `start_cursor_instance_with_account_switch(__default__, None)`。
- **涉及文件/模块**: `src-tauri/src/commands/cursor_instance.rs`、切片 `docs/implementation/0008-default-runtime-auto-switch.md`。
- **验证方式**: cargo check；开关开启且默认 Cursor 在跑、额度到阈值时日志出现「默认自动换号成功/失败」；多开不被本 tick 切换。
- **风险或注意事项**: 旧档案中「只扫多开」条文作废，以本条为准。

### F-015 续杯管家八模块（用户语言版说明）

> 这张卡是「续杯管家补完整」这一整块功能的**人话说明**。蓝图原文在 `docs/implementation/0010-renewal-butler-completion.md`（八个模块，每段按「原版是什么样 / 现在什么样 / 要改成什么样 / 验收怎么看」四段写）。本卡只留用户能读懂的部分，不写代码实现。
>
> **顺序偏离登记（诚实）**：本功能代码早于 wiki 完成，属顺序偏离，已登记；后续按「先写 wiki 再写代码」执行。
>
> 因此本卡是**事后补写的说明**，不是施工图；它不证明功能已好，也不改动已发生的事实。八个模块的真实好坏以磁盘上的 QA（`docs/qa/0010-renewal-butler-completion.md`）为准，见本卡末尾「当前状态」。

#### W-1 自动换号永不被偷关

- **这个功能替用户解决什么**：用户把「自动换号」打开以后，就不再想跟它较劲。以前的情况是：明明开着，用着用着就自己变回关闭了；用户会反复被「设置被偷偷改掉」这件事折磨。
- **用户眼里的边界**：开关是用户的东西。只有用户自己在界面上动手拨过，它才能变。程序自己读不到设置、读乱了、或者在换号过程中顺手回写——这些都不是「用户关掉了」，不许被当成用户关掉了。「我没动过它，它就一直是开着的」算好；任何一次「我没动，它自己关了」都不算好。
- **验收含义**：连续换号很多次，开关一路都是开着的；把某个设置文件删掉再开软件，它也不许自己写成关闭；三处存开关的地方说法得一致，不许互相打架；程序若真想偷偷关，得留下一条可被查出来的记录，而用户看到的仍然是开着的。

#### W-2 续杯管家自动换号真会自己跑

- **这个功能替用户解决什么**：用户要的是「续杯管家那套自动换号在 Cockpit 里真的能跑」。以前集成进来的那一套是半截的：有个口子在等别人来叫它，结果没人叫，等于没有；判断额度那一步还一直返回「没事」，跟真实额度没关系。
- **用户眼里的边界**：不用用户去额外开什么脚本、不用外部程序来推一把，Cockpit 自己到点了就该自己动；判断是否该换号必须看**真实额度**，不能永远答「不用换」；换到哪一步、上次成没成、还在不在冷却，得让用户看得见。还有一条硬边界：**续杯这套开关管续杯的号，默认页那套开关管默认页的号**，两套各管各的，不许互相覆盖、不许互相触发。
- **验收含义**：在不借助外部脚本的情况下，能亲眼看到一次真实的自动换号发生；它报的额度和账号页显示的一致；关掉默认页的开关，续杯这边照样能换；反过来也一样；续杯换完号，默认那边绑的号没被动过。

#### W-3 点一下不再卡住

- **这个功能替用户解决什么**：用户点一下，界面就得有反应。以前存在 20 秒以上的整段干等、后台还挂着一个几十秒不停轮询的常驻动作、进页面时同步跑外部探测、操作完再连着刷三遍全量——结果就是「点了没反应，或者要等很久」。
- **用户眼里的边界**：没事就别空转。确认到位了就立刻停，不要为了保险反复重写；后台那个常驻动作只在真的发现跟期望不一致时才动手；进页面先让用户看到画面，重活放到后面悄悄补。能接受的手感是「通常很快就有反应」，不是「固定等二十秒」。
- **验收含义**：一次换号从点到能用不再出现整段二十秒的等待；连着操作十次界面都能点得动；一次操作只完整地算一次状态采集；进续杯页的第一屏不被外部探测堵住。

#### W-4 按钮都不是摆设

- **这个功能替用户解决什么**：按钮按下去要有真事发生。以前有几个按钮点了只弹一句「功能复刻中，即将接入」；还有几个开关在界面上永远显示关闭，只是因为背后写着死值，并不是真实状态就是关闭。
- **用户眼里的边界**：按钮要么真干活，要么**明明白白告诉用户它现在不能用、为什么不能用**，并把按钮置灰；不许用一句漂亮话冒充「快好了」。开关显示的值必须是磁盘上的真实偏好；读不到就说「未知」，不许拿「关」来凑数。
- **验收含义**：拉号 / 换号 / 无感换号点下去能看到真的结果（池子里的号变了，或者 Cursor 那边真的生效）；开关显示的值在改了磁盘上的文件再刷新后能跟着变；「指定 Cursor 路径」指定了以后，后面的动作真按这个路径走；「推荐版本」得说得出这个版本号是从哪来的。

#### W-5 提示方式照老版来

- **这个功能替用户解决什么**：出事要看得见。以前集成版把什么都压成一句一闪而过的小提示，用户容易错过，也不知道进行到哪一步。
- **用户眼里的边界**：按原版规矩分两种场合。激活、卡密、重置、改密码、退出登录，以及各类失败——这些要有**正经弹框**，不论成功还是失败都要弹，用户点了才算过去。唯独「无感换号」顺顺畅畅跑完的主路**不要**再拦着用户点一次「确定」，改个账号文字、刷新状态行就够了。另外 Cursor 里面换了号也要有提示，页面上还要有一条跟着动作变的状态行。
- **验收含义**：那些非主路径的操作真会弹出框；无感换号成功不会再要求用户多点一次；换号后 Cursor 里能看到「账号已切换」的提示；操作过程中状态行的字是真的一段时间一个样，不是一句死话。

#### W-6 页面信息不滞后

- **这个功能替用户解决什么**：别的页面改了账号或注入状态，续杯这边要跟着变，不能让用户手动刷新才看得到。
- **用户眼里的边界**：一个刷新周期内能看到变化；主要靠「有事就通知」，定时刷新只做没通知时的兜底。定时刷新必须是**便宜**的，不得靠把重活缩短间隔来假装实时。
- **验收含义**：在别处改了账号/注入状态后，续杯页在一个周期内自己就变了；续杯页和账号页都能收到那个通知；定时刷新不会让 CPU/磁盘明显忙起来。

#### W-7 批量自动点击

- **这个功能替用户解决什么**：测试要能一下子点到站里所有能点的东西，而不是给每个按钮手工登记一遍。以前登记过的只占全部按钮的百分之一多点，绝大多数点不到。
- **用户眼里的边界**：页面上像能点的东西都被自动找出来，每个都有稳定的身份，可以被直接点到。危险操作（删号、退出登录、清数据、重置机器码、杀进程这类）**默认不许点**，要显式放行才行。原先手工登记的那些继续能用。每次点击都得留下「点到了什么、成没成、是不是被拦了」的回执。
- **验收含义**：可点覆盖率接近全部；危险操作的点法会被拒绝并说清为什么；老的那批登记标识一个都不失效；对着全站点一遍，能列出「点了哪些会卡死」的清单。

#### W-8 额度显示真额度

- **这个功能替用户解决什么**：额度要显示真的。以前界面上无条件写着「无限额度」，跟账号实际剩多少毫无关系，这是假数字。
- **用户眼里的边界**：额度和账号页用同一套算法、同一个来源，两处必须说得一样。查不到的时候就写「核实中」，**绝不允许**拿「无限额度」填空。只有当确定这个套餐本来就是无限的时候，才可以说无限，而且这个判断得有出处，不许写死。
- **验收含义**：续杯页和账号页对同一个号显示的额度一致；查不到的时候显示「核实中」而不是「无限额度」；拿一个本来不是无限的号来看，不会显示无限。

#### 当前状态（口径以磁盘为准，2026-09-13）

- 切片 0010：**`in-progress`，二元：没修好**。
- 模块一~六、模块八（G1–G4）：**静态已修好**。
- 模块七：H1–H3 **协议层通过**；**H4 全站扫未执行**（需要图形界面人工批量点，不许空跑冒充）。
- 真机未执行：A1–A5、B1/B3/B4、C1/C2、D 全项、E/F/G 真机、H1 覆盖率实证、H4。
- 门禁：`cargo check`、`npx tsc --noEmit`、`npm run build` 均 EXIT 0；指定单测 25 passed；`npm run tauri build -- --debug` EXIT 1（缺 `TAURI_SIGNING_PRIVATE_KEY`，非功能回归）。

#### 维持边界

- 「静态已修好」不等于「用户在自己机器上已经好了」；没跑真机的一律写未执行。
- 本卡不改任何既有切片结论，只补写；与本卡措辞不同的旧结论仍以原条目为准，本卡不作推翻。
- 正式 wiki 见 `.qoder/repowiki/zh/content/核心功能/续杯管家.md`（续杯管家的自然语言功能页，已写入真正的 repowiki，本卡仅作设计档案历史溯源）。

### 2026-09-13（补写 0010 八模块用户语言说明）

- **触碰功能**: F-015（新增持久说明卡）。
- **用户目标是否变化**: 否；按 AGENTS.md 硬规则 4「蓝图 → 蓝图 wiki → 代码」的要求，把八模块蓝图落成给人读的自然语言说明。
- **本次目的**: 让后续人与 Agent 先读到「用户要什么、什么算好、人怎么判断好了」三层意思，再谈代码。
- **实现手段**: 在 `docs/FORK-DESIGN-ARCHIVE.md` **末尾增量补写** F-015 一张卡（W-1～W-8 八节）+ 当前状态 + 维持边界；未删改既有内容，未触碰 `src/` 与其它任何文件。
- **顺序偏离**: 本功能代码早于 wiki 完成，属顺序偏离，已登记；后续按「先写 wiki 再写代码」执行。
- **涉及文件/模块**: 仅 `docs/FORK-DESIGN-ARCHIVE.md`（本轮唯一改动）；只读参考 `docs/implementation/0010-renewal-butler-completion.md`、`docs/qa/0010-renewal-butler-completion.md`、`docs/project-status.md`。
- **验证方式**: 状态口径逐条比对 `docs/qa/0010-renewal-butler-completion.md` 与 `docs/project-status.md` 第 57–64 行的实际文字后落笔；本轮未运行任何构建命令。
- **风险或注意事项**: 本卡是事后补写，不得被读成「功能已修好」；二元结论仍以磁盘 QA 为准——0010 整体仍是**没修好**。

