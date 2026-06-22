# Cockpit Tools — 会话简报

**日期**: 2026-06-22  
**任务**: 上游 v0.26.5 合并（保留 fork 改动）

## 合并结果

| 项 | 值 |
|----|-----|
| 集成分支 | `integrate-upstream-v0.26.5-20260622` @ `92cc6da0` |
| 上游基线 | `upstream/main` @ `d5ad5cea`（v0.26.5 + 4 commits） |
| Cargo 版本 | `0.26.5` |
| 源分支（合并前） | `cockpit-rotation-pick-20260620-v3-codex-upstream` @ `77a9cc3f` |
| 调试构建 | `target/debug/cockpit-tools.exe`（NSIS 打包因网络失败，二进制已编译） |

## 保留的 fork 能力（静态/编译已核对）

- Cursor 无忧切号：`pick_full_quota_account`、`is_cursor_transient_quota_error`、`close_cursor` profile 级
- 多开 pick / 配额 UI 修复 / NVIDIA Codex catalog + cliproxy xhigh→max
- Windsurf profile bootstrap（`bootstrap_windsurf_profile_by_launch`）
- `verify_cursor_switch_paths.py` PASS

## 历史 Cursor 验收 exe（合并前，仍作对照）

- `Desktop\Cockpit-nirvana-token-test.exe` SHA `F5256511…` @ `44321ebf`
- 合并后须用新构建重新点验 Play/多开后再更新「能用」基线

## 待用户点验

- GUI Play/多开切号、配额 UI、NVIDIA 双账号（后端链已编译通过）
