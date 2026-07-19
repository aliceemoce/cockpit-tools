# 失败经历档案

**维护规则**：用户判「非白 / 垃圾 / 重做」后，**本文件追加一条**（日期 + 失败物 + 根因 + 禁止再用的做法 + 下次必须换什么），然后才能重试。禁止重复已记录的失败链路。

---

## 2026-07-15 — 整棵换上游 src/ 拼装冒充同步
- **失败物**：分支 `-full` / SHA `1A0EC65E…` / `96ea7855`（Take upstream frontend… Keep fork Cursor）。
- **根因**：merge 后二次提交整棵 `src/` 改上游再嫁接 fork 后端；角标既非 fork 亦非线上。
- **禁止再用**：`git checkout upstream -- src/`；二次「整棵前端改上游」；在拒收 `-full` 上打补丁冒充 redo。
- **换法方向**：干净 `918980e9` 上 `git merge da0deca4`；手解冲突；Cursor 页=fork blob。

## 2026-07-14 — redo 交付仍露红 UNKNOWN 角标
- **失败物**：用户截图粉红 `UNKNOWN`；安装 SHA `D6C85620…`。
- **根因**：Dashboard/presentation 比对 `!== 'UNKNOWN'` 而 displayName 为 `'Unknown'`；CSS uppercase 显示 UNKNOWN；浮窗直出 planLabel。
- **禁止再用**：只保 CursorAccountsPage、其它入口仍打 Unknown。
- **换法方向**：`resolveCursorPlanUiBadge` 统一禁 UNKNOWN；pending→配额未查询。

## 2026-06-25 — F5 窗口自举 + 自复检全链路

- **失败物**：`manifest_shiro_window_verified`（3457 全过、3751、E01 63/64 等）；`window_first_f5` 导出碎屑（median ~1.2s，大量 &lt;0.5s）。
- **根因**：整集 vocals 上 F5 毫秒窗自举「白/其他」+ **同一模型**导出门控与复检；分不清白与其他角色，指标可过、内容混角色。
- **禁止再用**：`extract_shiro_window_first` + `recheck_export_purity` 自举自检当最终交付；`rebuild_manifest_from_wavs` 拼未过门控文件；`seed_discriminated` 放宽标白；全量 12 集在单集未过关前跑批。
- **换法方向**：先 **pyannote 讲话人分离 + 绑定白 speaker**（`manual_picks` / `shiro_pure_v3` 路径），再裁段；复检不得与导出同一 `label_window`；过关前只交**完整音频**且助手打开并证实播放。

## 2026-06-25 — 交付形态错误

- **失败物**：打开 Explorer 文件夹、CSV、Edge、`e01_calibration.html`、无播放证据的 m3u。
- **根因**：把「能点开文件」当交付，未打开具体音频、无播放证据。
- **禁止再用**：HTML 试听页；指定 Edge；仅打开目录/清单代替播完整音频。
- **换法方向**：`Start-Process` 具体 `.wav` + UIA 证实播放器与播放状态。

## 2026-06-25 — 跳过「先做一个」

- **失败物**：全量 batch 在 E01 未过关时跑完并写 summary。
- **根因**：把脚本跑通当完成，跳过单集关门。
- **禁止再用**：E01 未批准前跑 `run_window_first_batch` / 全量复检并标 verified。
- **换法方向**：单集 → 一条/一组完整白音频 → 用户批准 → 再扩。

## 2026-06-25 — 二十余次「根因已修」叙事

- **失败物**：每次换说法（缺负类、bootstrap 灌池、margin 阈值…）仍交同类 manifest。
- **根因**：在同一无效链路上调参，宣告完成。
- **禁止再用**：无链路变更的「已修复/已验收」；用 pass_rate 证明是白。
- **换法方向**：失败写入本档 → 换分离链路 → 只交完整音频+播放证据等用户批。

---

## 追加模板（拒收后由 Agent 填写）

```markdown
## YYYY-MM-DD — 简述
- **失败物**：
- **根因**（日志/路径/用户拒收点）：
- **禁止再用**：
- **换法方向**：
```
