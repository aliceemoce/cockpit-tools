# 会话快照 — 2026-06-30

## 当前任务（已完成本轮）
1. **规则强化**：工作区 + 全局 `cockpit-deploy-github-history.mdc`（禁止裸 debug 覆盖安装、须 GitHub 同步、对比口径）
2. **GitHub 上传**：fork 数月工作区改动 + HR 主档 + 规则 + 设计档案 → commit `0149a597` push
3. **覆盖前任务清单**：见 HR-20260630-007；勿用 git HEAD 冒充用户基线

## 安装 exe 状态
- 坏包（已替换）：SHA `96B2738B…`（裸 `cargo build` debug，WebView 网络错误）
- 当前安装：SHA `9DF4DF9E…`（`npm run tauri build --debug` 嵌入 dist）
- 本轮 commit 后**未**重编译覆盖安装（删除 `cursor_profile_theme` 后 SHA 会变）

## 基线
- 仓库：https://github.com/aliceemoce/cockpit-tools
- 分支：`integrate-upstream-v0.26.5-20260622` @ `0149a597`
- 历史要求：`.cursor/user-history-requirements.md`（HR-20260630-001～008）
- 全局技能：`~/.cursor/skills/cockpit-cursor-switch-main-js/SKILL.md`
