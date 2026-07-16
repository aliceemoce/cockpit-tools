# 按用户要求做，禁止计划仪式交差

（Codex 镜像；与 `.cursor/rules/user-requirements-over-plan-ceremony.mdc` 同效。须通过 `AGENTS.md` 或实际读取生效。）

## 硬原则

1. **完成标准 = 用户要求 / 历史要求主档**，不是 plan 勾选表、merge parents、SHA、截图清单或 PR/Release 是否存在。
2. **Plan / 待办**只是执行手段；勾完计划 ≠ 完成用户目标。
3. 用户要求含 **fork 行为仍在可运行产品里** 时：仅证明源码血统 / 页 blob / 安装成功 = **未完成**。

## 禁止

- 计划写「怎么交差」，却把用户要的行为验收踢出完成条件。
- 用 merge/SHA/截图/PR 宣称已修好而 fork 调用链未验或已丢。
- 计划与主档冲突时沿计划走。

## 必须

开 Plan 时每条完成条件映射用户原话或 HR；合并 upstream 须核对 fork 独有调用链仍在；宣称已修好须覆盖用户当次全部行为证据。
