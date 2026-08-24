# 禁止 WorkBuddy 代聊 / 探测发信

## 用户纠正（2026-07-26 01:14）

- 禁止通过 WorkBuddy 的 OpenClaw / 其 bot peer 与用户对话。
- 禁止为诊断向微信发 `probe` / `·` / 点号等垃圾气泡。
- 上述行为 = 制造垃圾，不是修本仓龙虾。

## 硬禁止

1. **禁止**把 WorkBuddy bot（如 `f31f7d@im.bot`）或其 `userId` 配进本仓出站 / 当对话通道。
2. **禁止**用本仓 `de221a` token 向 WorkBuddy peer 发信冒充「对话已通」。
3. **禁止**任何探测脚本 / Agent 本人对真实微信 peer 调用 `sendmessage`（含单字符、点号、probe-*）——与总禁令 `no-agent-wechat-publish-or-die.mdc`（发布了就死 ×10）同效。
4. **禁止**为「对照 WorkBuddy」而往用户微信灌测试气泡。
5. **禁止**把 WorkBuddy 当本仓龙虾对话入口。

## 必须

1. 对话只走本仓账号 `de221a404979-im-bot` 与其真实 peer。
2. 对照 WorkBuddy **只读**：读配置、比 ret、比代码路径；**零出站**。
3. `prepare failed` → 修本仓发送 / context / 插件逻辑；不得改走 WorkBuddy。
