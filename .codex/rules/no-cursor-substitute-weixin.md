# 禁止 Cursor 代替用户 ↔ 微信 OpenClaw

## 事实（2026-07-22）

用户只在 **Cursor IDE** 说过「任务完成看任务，不是看有没有回复」。用户 **从未** 在微信对 OpenClaw 说过这句话。随后微信 OpenClaw 却把「任务完成≠有回复」等 Cursor 侧措辞发出——等于 Cursor 污染/代替了用户与 OpenClaw 的对话，任务链被劫持。

## 硬禁止

1. **禁止** Cursor Agent（本 IDE 对话）以任何方式 **代替用户** 与微信 OpenClaw / ClawBot 对话（含：代发、代问、代答、把本窗结论塞进微信出站）。
2. **禁止** 把 **仅发生在 Cursor 窗** 的规划、纠正、元评论、状态叙述写入会被微信 ACP **当作用户权威并复述出站** 的内容路径后，任其原样发到微信。包括但不限于：把 Cursor 窗原话写进主档后，ACP 再在微信里复读成「用户要求」。
3. **禁止** `openclaw agent --deliver`、`message send`、直连 `sendmessage`、临时脚本、向微信 ACP session **注入** Cursor 窗消息，冒充用户微信入站或助手微信出站——**含任何探测/点号/单字**（总禁令：`no-agent-wechat-publish-or-die.mdc`，发布了就死 ×10）。
4. **禁止** 微信出站出现 Cursor 元话语：如「任务完成≠有回复」「只读核对 ACP」「正在写入主档」「先对齐权威任务与中断点」等 **仅 IDE 侧** 措辞（除非用户 **微信入站原话** 含该内容）。
5. **禁止** 经 WorkBuddy OpenClaw / bot 代聊或代发。

## 允许

- Cursor 在本机修通道、清 ACP 僵尸、改补丁、盘点磁盘交付物——**结果写在 Cursor 窗**，不得当作微信回复发出。
- 用户 **微信入站** 触发的 OpenClaw ACP 执行真实任务；出站只允许：任务执行结果（如 `MEDIA:`）、或对 **该条微信入站原话** 的必要极短确认——不得夹带 Cursor 窗闲聊。

## 污染后处理（源头）

1. 从 **OpenClaw / ACP session 与 pending 出站** 删除被污染的助手回合（出站源头），**禁止**以「帮用户在 PC 微信 UI 里删聊天记录」冒充清源。
2. 已发出的微信侧气泡：插件无撤回接口则如实标明 **OpenClaw 侧无法召回已投递气泡**；清源仍以 session/sanitize/防再发为准。

## 验收

用户微信里看到的 OpenClaw 回复，必须能追溯到 **同一会话的微信 inbound**，且正文不来自 Cursor IDE 对话独有结论。
