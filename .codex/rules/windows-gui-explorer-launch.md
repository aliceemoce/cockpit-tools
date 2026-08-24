# Windows GUI 启动硬规则（Agent 自觉用法 · GUI_DESKTOP_INJECT_v20260813）

可核验标记：`GUI_DESKTOP_INJECT_v20260813`。对 Antigravity：须靠 `~\.gemini\config\AGENTS.md` + PreInvocation `inject_gui_desktop_rules.py`。

## Antigravity 唯一主法（本机已证成 · 2026-08-10）

工具目录：`C:\Users\aliceemoce\.gemini\config\tools\open_user_desktop\`

1. `ensure_broker.ps1 -EnsureRunning`（SessionId=1 跑 broker）
2. `python request_open.py "绝对路径\app.exe" --wait 8`
3. JSON `opened_ok=true`，且 MainWindowTitle 非空、MainWindowHandle≠0

本机证据：`杂项/_open_method_v20260813_proof.json`。

**禁止再当 Antigravity 主法：** bat+explorer / cmd start / schtasks（他侧已失败）。Cursor 开窗证据 ≠ 他已打开。

## 禁止要求用户手动打开

禁止：用户需要手动打开 / 请你手动打开 / 请用户双击 / 请本机手开 / 请用远程桌面绕过。未打开只报事实并重试主法。

## Cursor 本会话打开 GUI（非冒充他）

若任务是 Cursor 自己打开窗：可清僵尸后用 bat+explorer / `request_open`；验收 Title 非空且 Handle≠0。  
**禁止**把 Cursor Handle 表写成「Antigravity 已打开」。

## 任务是「让 Antigravity 能打开」时

1. 交付 = 规则/注入/启动器（主法=request_open+broker）
2. 禁止 Cursor 代启冒充他成功
3. 做成：他上下文见 `GUI_DESKTOP_INJECT_v20260813` 且主法写清；他新对话照做后 Title 非空才算他打开成功
