# Cockpit UI 自验规则（Agent 必读）

> 适用：Cockpit Tools（Tauri + WebView2）及本 fork 一切 UI 验收。  
> 与 `aliceemoce/image-clicker-bot` 同一思路：**UIA Invoke，不抢前台，不挪鼠标。**

---

## 禁止事项

1. **禁止**向用户索要肉眼确认（「你看到弹窗了吗？」「请切到前台」等）。
2. **禁止**用 `desktop-touch` 的 `mouse_click` / `keyboard` / `focus_window` 做验收（SendInput / 抢焦点）。
3. **禁止**用 `uiautomation` 的 `activate_window` 做验收（会抢前台）。
4. **禁止**用 PowerShell 坐标脚本（`ui-click-test.ps1`、`verify-*.ps1`）作为通过依据。
5. **禁止**仅凭 `perform_action` / `click_element` 返回 `success: true` 宣称 UI 已变化——**必须**有截图或 UIA 树 diff 证据。
6. **禁止**点击账号卡区域（约 y 800–1100 的卡片行）——会触发切换账号。

---

## 必须使用的 MCP

| 用途 | MCP | 工具 |
|------|-----|------|
| 列窗 / 找控件 / Invoke 点击 | **`user-uiautomation`** | `list_windows`, `find_elements`, `perform_action` |
| 截图（仅裁目标窗，不抢前台） | **`user-uiautomation`** | `capture_screenshot(element_id=主窗 elementId)` |

底层库：`uiautomation`（与 `image-clicker-bot` 相同），**不是** `desktop-touch`。

`lazy-mcp` 若未连接，直接调 `user-uiautomation`，不要回退到鼠标层。

---

## 不抢占前端的点击

1. `list_windows` → 主 Cockpit：**1942×1256** 的那个（非 375×435 悬浮窗）。
2. `find_elements(element_id=主窗, name="…")` → 按 **Name** 定位，不靠屏幕坐标。
3. `perform_action(element_id, action="click")` → 依赖 **InvokePattern**；**不要**先 `activate_window`。
4. 两个「设置」必须区分：
   - 侧栏：`name="设置"`, `className` 含 `nav-item`，约 **y≈1300**
   - 工具栏（#7 入口）：`name="Cursor 设置"`, `className` 含 `btn`，约 **y≈820**

---

## 不抢占前端的截图

1. **只截 Cockpit 主窗口**：`capture_screenshot(element_id=<主窗 elementId>, highlight=false)`。  
   不要截全桌面（除非对比需要），不要为截图调用 `activate_window`。
2. **每个 UI 动作前后各截一张**（before / after），Agent 自己读 base64 PNG 判断变化。
3. 若 UIA 树里有新控件（如弹窗标题、`选择`、`恢复默认`），可与截图交叉验证；仅有 `success: true` 不算数。
4. 截图失败或 before/after 无可见差异 → 记为 **未验证**，不得写「已完成验收」。

---

## #7（QuickSettings 路径行）验收步骤

1. 确认在 **Cursor 账号页**（工具栏有 `添加账号`、`Cursor 设置`）。
2. **before**：`capture_screenshot` 主窗。
3. `find_elements` → `Cursor 设置` → `perform_action(click)`（不 activate）。
4. **after**：`capture_screenshot` 主窗。
5. 在 after 截图或 UIA 中确认：
   - 出现 QuickSettings 蒙层 / 标题 **Cursor 设置**
   - **Cursor 路径** 区块存在
   - 路径行旁有 **选择** + **Download 或 RefreshCw** 图标（非标题栏上的安装按钮）
6. 未看到上述任一项 → **#7 UI 未通过**，只可声称代码已改、UI 未证实。

---

## 表述规范

- ✅「已截图 before/after，after 中可见 …」
- ✅「UIA 点击已执行，但 after 截图无变化 → 未验证」
- ❌「点击成功，任务完成」
- ❌「请你看一下是否弹出弹窗」

---

## 参考

- 私有库：`aliceemoce/image-clicker-bot` — `uia-only`，`LIBRARIES.md`
- 源码：工具栏 `QuickSettingsPopover` `aria-label` = **Cursor 设置**；侧栏 `SideNav` = **设置**（不同入口）
- 弹窗：`createPortal` → `.qs-overlay`（z-index 9999）；WebView 内 modal 控件 UIA 可能很浅，**截图是最终兜底**
