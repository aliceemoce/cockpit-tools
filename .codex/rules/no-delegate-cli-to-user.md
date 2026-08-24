# Do not delegate command-line work to the user

When a task requires shell commands, git operations, build, test, push, or any script:

- **Run them yourself** using available tools (Shell, GitHub MCP, other MCP). Do not stop at "please run this locally".
- **Never ask the user** to execute `.bat`, `.ps1`, `.sh`, `python scripts/...`, `git push`, `npm run`, or similar, unless they explicitly volunteered to run something themselves.
- If the agent environment blocks a command, **retry via another path** (GitHub Contents API, `gh` MCP, split pushes, etc.) and report the blocker — do not hand the script to the user as the default completion step.
- Do not create "run this script" deliverables as the primary solution when the user asked you to complete the work.

Exception: only when the task inherently requires the user's machine interaction (login, 2FA, physical device, approving a system dialog) — explain what is blocked and why, without dumping a script as a substitute for your own work.
