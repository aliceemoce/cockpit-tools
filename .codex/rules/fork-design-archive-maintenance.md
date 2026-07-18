# Codex Fork Design Archive Rule

This file is the Codex-side mirror of the fork design archive maintenance rule. It is not a replacement for `AGENTS.md`; Codex must reach this rule through `AGENTS.md`, automation prompts, or explicit reading during a task.

## Required Before Any Change

Before any manual fork work, automation task, code change, document change, build, release, or upstream merge, read:

- `AGENTS.md`
- `docs/CURSOR-FORK-SCOPE.md`
- `docs/FORK-DESIGN-ARCHIVE.md`
- `.cursor/project-brief.md`
- `.cursor/rules/cockpit-feature-registry-and-rule-evolution.mdc`

## Required After Any Actual Change

If the task changed code, docs, rules, build artifacts, release state, automation prompts, or delivery records, update `docs/FORK-DESIGN-ARCHIVE.md`.

Use feature-based maintenance:

- Update the touched feature card when code design, verification, risk, or boundaries changed.
- Append an increment record with touched feature, user-goal change status, purpose, implementation means, affected files/modules, verification, and risk.
- Do not rewrite the whole archive unless the user goal itself changed.
- Do not update the archive for a no-op inspection or upstream check that made no changes.

## Natural Language vs Code Design

- Natural language design records the user's purpose, boundary, and acceptance meaning.
- Natural language design must stay directly tied to the user's requirement and must not be rewritten into a different goal because implementation changed.
- Code design records implementation means, modules, dependencies, verification, and risks.
- Code design may evolve when a better implementation is found.

## Implementation Source Preference

When adding or replacing implementation approaches, prefer project MCP context, upstream behavior, official documentation, mature libraries, or original-author patterns. Avoid inventing private terminology, private protocols, or ad hoc "languages" when an established approach exists.

## Codex Boundary

`.cursor/rules/` is a Cursor-side rule mirror. Codex must not rely on Cursor rules as an automatic enforcement mechanism. Codex-side enforcement is through `AGENTS.md`, this `.codex/rules/` file, automation prompts, and the actual files read in the task.
