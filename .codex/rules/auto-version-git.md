# 自动版本 Git 留档（用户级 Cursor Hook）

## 行为

Agent 回合 `stop`（`status=completed`）时，`~/.cursor/hooks/auto-version-git.ps1`：

1. 从 payload 的 `workspace_roots` / `cwd` 等字段取工作区（**禁止**误用 hook 脚本所在目录当工作区）
2. 用 `git rev-parse --show-toplevel` 找 Git 根（兼容中文路径；不再只靠 Test-Path 找 `.git`）
3. 读根目录 `VERSION` / `version`（semver）
4. 若尚无 tag `vX.Y.Z`：`git add -A` → commit → **annotated tag** →（默认）`git push` + push tag
5. 已有同名 tag → 跳过

## 补打历史 tag

某仓漏打 tag 时，在仓根执行：

`python scripts/补打版本标签.py --push`

按 `VERSION` 文件变更历史，为每个尚未存在的 `vX.Y.Z` 在对应 commit 补打标签。

## 配置

- `~/.cursor/hooks/auto-version-git.config.json`：`enabled` / `push` / `tag_prefix` / `remote`
- 单仓关闭：在该仓建 `.cursor/no-auto-version-git`
- 预演：环境变量 `CURSOR_AUTO_VERSION_DRY_RUN=1`
- 强制指定工作区：`CURSOR_AUTO_VERSION_WORKSPACE=C:\path\to\repo`
- 日志：`~/.cursor/hooks/auto-version-git.log`

## Agent

发版时仍须 bump `VERSION` 并写 `CHANGELOG`；hook 负责在无对应 tag 时把可追踪树打成该版本快照。不把「只改了文件、VERSION 未 bump」当成新版本。
