# Cockpit Tools — Agent 项目规则

## 四条硬规则

1. 当前版本必须写明（当前 1.3.22）。
2. 打包必须含 UI。
3. 应用内操作要完好；只用应用内操作操作本应用，做不到就改应用内操作的代码和操作种类，应用内操作可以多种。
4. 你要看或制作的自然语言功能蓝图按仓库 repowiki 写：按 repowiki 写代码，用户说的先改 repowiki，再改代码，按「蓝图 → repo wiki → 代码」渐进，不能凭空写代码；若仓库没有 repowiki，则不写 wiki，只按蓝图写代码。

## 开工必读顺序

1. 读 `.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc`：明确能用/不能用的交付边界，禁止把 debug 包、拼装包、截图、PR 冒充完成。
2. 读 `docs/FORK-DESIGN-ARCHIVE.md`：持久功能设计档案，只允许增量补写，不允许推翻既有切片结论。
3. 读 `docs/CURSOR-FORK-SCOPE.md`：当前 fork vs upstream vs 开发仓的职责边界。
4. 读 `.cursor/user-history-requirements.md`：用户历史未决事项，任何新任务不得与其冲突。
5. 读 `docs/project-status.md`：当前版本、正在推进的切片、仍未宣称完成的事项。
6. 读本文件的「四条硬规则」「单个切片交付清单」「完成定义」「自动化门禁命令」「状态机」。

## 单个切片的交付清单

任何切片必须同时满足以下 4 类产物，缺一项只能记为「进行中」：

- 规格：`docs/implementation/NNNN-slug.md`，至少写清「本次只做什么 / 明确不做什么 / 验收标准 / 涉及文件模块 / 上下游依赖」。
- 过程：`docs/development/NNNN-slug.md`，从第一行代码前开始持续记录「已完成行为 / 偏离规格 / 失败与修复 / 关键命令 / 已知问题」。
- 代码：代码变更范围必须与 implementation 中声明的文件一致；多改或少改的文件必须在 development/review 中解释。
- 验收：`docs/reviews/NNNN-slug.md`（Standards 轴 + Spec 轴）与 `docs/qa/NNNN-slug.md`（通过/失败/未执行逐项）。

## 完成定义

一个切片被视为完成，必须同时满足：

1. 实现与规格一致：代码真实满足 implementation 中写的验收项，不是 TODO/stub/mock。
2. 自动化门禁通过：下文列出的自动化命令均通过，或在 review 中逐项说明已知不适用原因。
3. 四件套齐全：implementation / development / review / qa 文档都存在，且内容非空壳。
4. 偏离与问题可追溯：任何偏离规格、未执行、无法验证的项都必须写入 development 与 qa，不能空口声称“没问题”。
5. 结论二元：对用户只能说「已修好」或「没修好」。是否完成由磁盘证据与编译/实测结果决定，不把“等用户确认”作为完成门槛。

## 自动化门禁命令

所有切片在声称完成前，至少执行并记录以下命令的结果：

- Rust 静态检查：
  - `cargo check --manifest-path src-tauri/Cargo.toml`
- 前端类型检查：
  - `npx tsc --noEmit`
- 前端生产构建：
  - `npm run build`
- Tauri 完整构建（如本轮目标涉及 GUI 交付）：
  - `npm run tauri build`（或至少 `npm run tauri build -- --debug` 用于本机验证）
- 与切片相关的单测 / 集成测试：
  - 若仓库已有测试脚本，则按项目约定执行；没有则在 qa 中显式记为「未执行」及原因。

任何一项失败，除非 review 明确记录为“与本切片无关且已确认回归来源”，否则该切片不得视为完成。

## 状态机

### 切片状态

- `pending`：已立项但未创建 implementation 规格。
- `spec-ready`：implementation 已写清范围，未开始代码改动。
- `in-progress`：正在编码，development 日志持续更新。
- `self-check`：代码完成，正在跑门禁命令与自查 review/qa。
- `done`：满足「完成定义」全部条件。
- `blocked`：被外部依赖（上游接口、真机环境、凭据、上游 release）阻塞，必须在 development 中写明阻塞点与重新触发条件。

### 项目级约束

- 用户明确要求的功能优先级高于任何计划勾选；计划不得反过来抵消用户要求。
- 禁止整棵替换 `src/` 冒充同步；禁止 debug 包当基线；禁止第三套拼装 UI；禁止 PR/截图/SHA 冒充真实可运行产品。
- 涉及架构决策（状态管理、存储模型、跨进程通信、核心依赖替换等）必须补 `docs/adr/NNNN-decision-slug.md`，不允许只留在聊天里。
