# 额度不足重试 + 「下一」「没有」+ 进度关键词

## 用户纠正（2026-07-25）— 仅用户原话

- **对话不支持你说的内容**（截图：Weixin + `You've hit your usage limit`，窗里看不到跟帖气泡）：用量弹窗后该对话接不住 stop-hook 跟帖；禁止再把「stop-hook 已 EMIT / 单测过」说成做成。
- **做成**：没做完就继续做完；做完了就不该再出现用量/Pro 提示；要检查还有没有弹窗，**没有弹窗才算做完**；现在还出现 = 未完成。对齐 IDE 可对话号再派是手段，不是终点。
- **Conversation data missing**（用户原话）：问题在这种情况**为什么会出现**；说云端但以为是本地对话；要**解决所有对话**的问题；纠正「怎么想的是修复」。
  - **2026-07-25 再纠正**：事实=**部分对话**无法延续 + 指定报错；「未上传云端」只是猜测不得当事实；任务=修好这部分对话的报错，不是上传问题、不是切号。
  - 程序侧（非原话）：个别会话可清 `aborted`/死 generation 指针；`latestChatGenerationUUID` 不得伪造；禁止只改一条就宣称全部解决。
- **没触发重试，重做**：用量弹窗打断半截实做时，须能触发续做（含 UI toast 不进 transcript 的情况）。
- **5 词加入 hook**：没做完 / 就继续 / 已写进主档 / 未完成 / 本回合加入hook。
- **2026-07-26 再加**：失败 / 没跑通（未完成已在上列）；助手正文含这些词须 force-continue。
- **2026-07-26 再加**：之后 / 偷懒 / 做到一半；分步骤推后或中途收工须 force-continue。
- **2026-07-26 再加**：继续；助手正文含此词须 force-continue。
- **2026-07-26 再加**：尚未 / 缺口 / 仍是 / 还没有 / 即可；口头留缺口或「即可…」交差须 force-continue。
- **2026-08-05 纠正（Plan/Agent）**：用户原话——助手先前说「切模式要你点确认」**不符合实际**；**实际是可切换 plan，不切换 agent**。stop-hook 须识别「停在 plan / 最后 SwitchMode=plan / 只写计划正文收工」并跟帖强制 `SwitchMode(agent)` 后执行；规则见 `self-plan-before-execute.mdc`。

**禁止**：把用户没说过的词、标签、禁令写成「用户纠正」或「用户原话」。实现细节不得冒充用户要求。

## 用户要求

- 末条用户含「下一」或「没有」→ 禁止停（仍走 stop followup，仅当 Cursor 接受跟帖时有效）。
- 用量 / Connection+Pro / Internal 弹窗 → **程序侧对齐 IDE 可对话号并续派**，禁止当买 Pro 终点。
- 禁止「停任何一个对话都重试」（无 toast 证据的空 error 不得无条件续）。

## 机制分工

| 路径 | 做什么 | 不能指望什么 |
|---|---|---|
| IDE `stop` hook | 能跟帖时吐 `followup_message`；扫 `turn_ended.error`；防误触 | **用量弹窗对话拒收跟帖时无效**（用户原话：对话不支持你说的内容） |
| `_sync_cli_local.py` + 看板 `工作区汇报.py` | 对齐 IDE 可对话号、`chat_ok`、Automations 遇用量再试 | 不能代替「弹窗对话里一定出现跟帖气泡」 |

## Hook 仍保留（有限）

- 脚本：`~/.cursor/hooks/continue-if-next-or-meiyou.ps1`
- 有 toast 字样 / `turn_ended.error` → 尝试 followup；stop 早于落盘时短等再读 transcript
- **2026-08-05**：`composer_mode=plan` / 末次 `SwitchMode=plan` / 计划正文收工 → `plan-stuck` 跟帖，强制 `SwitchMode(agent)` 后执行（对应用户：可切 plan，不切 agent）
- 这一回合 `error`/`aborted` 且报错/返回含灰条字样（如 hit your usage limit）→ 吐 auto-retry。
- **用户关键词**（扫末条助手正文，`completed` 与 `error`/`aborted` 都扫）：下一步、下一刀、盯着日志、有在执行；没做完、就继续、已写进主档、未完成、本回合加入hook；失败、没跑通、之后、偷懒、做到一半、继续；尚未、缺口、仍是、还没有、即可。命中 → 吐 force-continue。禁止用「自己已经发过英文气泡」当发或不发的原因。
- 末条**助手**正文命中进度/未完成词 → force-continue。含：`下一步` / `下一刀` / `盯着日志` / `有在执行`，用户指定 5 词：`没做完` / `就继续` / `已写进主档` / `未完成` / `本回合加入hook`，以及 2026-07-26：`失败` / `没跑通` / `之后` / `偷懒` / `做到一半` / `继续` / `尚未` / `缺口` / `仍是` / `还没有` / `即可`
- 间隔禁止长时间 Sleep 拖死 stdout

## 硬禁止

1. 禁止用 hook 单测 / `EMIT` / `FAILED=0` 宣称弹窗对话已自动续跑。
2. 禁止叫用户手动点 Upgrade to Pro / 重试当终点。
3. 用量症状出现 → 当场 sync 对齐可对话号 + Automations/CLI 续派到有证据。
4. 助手正文出现上述 5 词之一却结束回合 = 违规；hook 须续派。
5. **2026-07-25**：stop-hook / ensure-chat-ok **禁止** `cursor_force_default_switch`（关 Cursor 切号会换掉全部对话）；只允许 IDE→CLI 写穿。
