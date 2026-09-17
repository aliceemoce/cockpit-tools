## 2026-09-10 额度/刷盘/alone（权威 · 待做待核实）

- **待做**：T1 hold刷盘（源码已改未上桌面）；T2 auto_switch偷翻；T3/T4 额度+默认自动换号；T5 FO关后CO alone；T6 抽样格式进拉号主链
- **待核实**：V1 remaining恒100（已证实）；V2 ~2.5s热替换；V3/V4 alone；V5 抽样格式
- **release 包**：exe `target\release\cockpit-tools.exe`（101112832，2026-09-11 18:16:29）；**NSIS** `target\release\bundle\nsis\Cockpit Tools_1.3.22_x64-setup.exe`（64874174，2026-09-11 18:16:29，SHA256 `FB09DBC5E129741C0AB9AB9B33A3C7A545EE65F43C291A55EEFB78CF78E79E4F`）；先编 batch（CARGO_EXIT=0）；日志 `_build_logs/tauri_build_0009_nsis_r2.txt` EXIT=1=缺 `TAURI_SIGNING_PRIVATE_KEY`；MSI 未打；**没修好/待真机**
- **切片**：0009 · **二元**：**没修好**
## 2026-09-09 无忧 Cursor 切号 / 拉号 / 子代理（续做）

- **用户原话**：继续（承接六问与无忧面板）
- **已核**：
  - 覆盖安装后 deep link 点「切换」：界面弹「切换成功」；audit 为 inject/launch 成功、**stick_fail**（读到旧号）——接线通，粘号未过，切号不得标完全成功
  - 原版解包 `cursor:cloud-pull` 亦返回「Cursor 云端号池即将开放」；Cockpit `nirvana_cursor::cloud_pull` 与 IPC `handle_cloud_pull` 已统一硬失败文案（未接入、不入库）；**真拉号 API 入库仍缺**
  - 查云端拉号 / 查 Proxy Qoder / 续查 Proxy Qoder：三个子代理均因额度中断，无调研正文；额度恢复后仍由子代理续做
- [x] Cursor 无忧「切换」接真切号 + deep link 点验（wiring；粘号另项）
- [x] 拉号诚实硬失败（主路径 + IPC）；禁止「即将开放」软空
- [ ] 粘号 stick_fail 修通
- [ ] Nirvana Proxy 无感换 Qoder（子代理续查后再接线）
- [ ] 入口层级 / Cockpit 画风 / nirvana-full-port 分页面
- **覆盖说明**：六问节勾选以本条为准增量；旧六问正文保留

## 2026-09-08 底栏绑启动器 + 无 PowerShell 蓝窗（当前权威）

- **用户原话**：1.让底栏绑定你做的；2.不要有powershell弹窗的；继续
- [x] MyDockFinder `ico.ini` `[ico33]` → `...\CursorXuBei\CursorXuBei.exe`
- [x] `scripts/templates/xubei_updater.py` + 安装目录 `updater.py`：无 powershell/cmd 启动
- [x] `scripts/sync_xubei_renewal_auto_update.py` 同口径
- [x] 实测：`pythonw updater.py` 退出 0；日志 16:51；进程 14.3.6；powershell/cmd 计数未因本次上涨
- **覆盖说明**：同日「获得全部代码」正文保留；**本回合以本条两句用户原话为准**

## 2026-09-08 获得全部代码（已被同日「底栏绑启动器 + 无 PowerShell 蓝窗」覆盖主线 · 正文保留）

- **当前权威任务**：获得全部代码
- **完整代码标准（保留）**：完整代码 = 手里有能编回几乎一模一样、代码量相差极少的程序的能力；解包 / 反编译 / 可运行 / `cargo check` / 已接线 / 碎片 HTML ≠ 完整代码；一元只认是 / 否
- **禁止**：禁止「不含 Pro 算是」；禁止空 stub 冒充已有能力
- **Pro 说明（文首）**：Pro 明文已可从 `CursorXuBei-v14.3.5.exe` 字符串表抠出（i3 / isPure / tab / ext_verify / Anthropic 等），正在接入 `inject(pro=True)`；**无 Pro 仍 = 无全部**
- **本回合钉死**：三程序合计须达「几乎同款」能力；管家体量/结构、助手前端原版与体量、Pro 接入均未完成前，合计一元仍否
- [ ] 管家：体量/结构达几乎同款
- [ ] 助手：前端原版 / 体量 / updater 达几乎同款
- [ ] Pro：字符串表明文接入 `inject(pro=True)`（无 Pro = 无全部）
- [ ] 三程序合计一元仍否 → 直至全部达完整代码标准
- **覆盖说明**：同日「续杯管家自动更新」及更早「自动更新三程序」曾被本条覆盖；**本条主线已被同日「底栏绑启动器 + 无 PowerShell 蓝窗」覆盖**；正文保留不删

## 2026-09-08 续杯管家自动更新（用户：又没自动更新 / 为什么没自动完成 / 继续 · 已被同日「获得全部代码」权威覆盖 · 正文保留）

- **用户原话**：又没自动更新，让你做好几次了，怎么回事；所以为什么没有自动完成这个事情；继续
- **本回合钉死**：点图标须下载/安装/覆盖；做成 = **进程与界面已是最新**，不是仅报告/磁盘有包
- **根因（已核）**：14.3.5 已落盘，进程仍跑 14.3.4（直接开版本号 exe，绕过启动器）
- [x] 加固 updater / sync（落后进程强制换新）
- [x] 进程与界面验收 **v14.3.5**（`scripts/_xubei_ui_14_3_5_verify.png`）
- [x] 三程序总同步：管家 14.3.5 / 助手 10.0.3 / 无忧 2.8.6，报告 ok
- [x] 点图标启动器再实测：杀进程后经 `CursorXuBei.exe` 重开；`click_update_launch.log` 2026-09-08 13:48 新行；进程 `CursorXuBei-v14.3.5`
- **覆盖说明**：本条曾为同日自动更新权威；**已被同日文首「获得全部代码」覆盖**；不以本条顶替完整代码

## 2026-09-08 三程序完整代码 · 继续（已被同日「获得全部代码」权威覆盖主线 · 正文保留）

- **用户原话**：继续
- **本回合「继续」钉死（权威）**：**非 Pro 缺口续做**。助手 `__LOGBODY__` 运行时替换已接 + `cargo check` 退出码 0；前端碎片已挂入 `frontend/extracted_fragments/`（非原版）；管家无感缺路由已补（pending-resume / resume-done / reset-machine / apply-account；toomany-check 明确未实现）。**Pro 硬阻塞保留**。**三程序合计完整代码仍否**。
- **承接标准（权威 · 禁止两套口径）**：完整代码 = 手里代码**具备**编回几乎一模一样、代码量相差极少的程序的能力；解包 / 反编译 / 可运行 / `cargo check` / 已接线 / 碎片 HTML ≠ 完整代码；一元只认是 / 否
- **本回合状态（2026-09-08）**：
  - 续杯管家：无感本地路由已补；体量仍远小于 35.69MB → 一元 **否**
  - 续杯助手：LOGBODY 已接；碎片已挂；一元 **否**
  - 无忧业务包：一元 **是**；**三程序合计仍否**
  - **Pro**：硬阻塞保留
- [x] 助手：`__LOGBODY__` / EH 占位填充 + cargo 留证
- [x] 助手：`ui_extracted` → `frontend/extracted_fragments/`（标明非原版）
- [x] 管家：无感缺路由（真接 / toomany 硬错误）
- [ ] 管家：体量/结构未达几乎同款 —— 一元仍否
- [ ] 助手：前端非原版 / 体量 / updater —— 一元仍否
- [ ] Pro：材料缺失硬阻塞保留
- [ ] 三程序合计一元仍否
- 落点：`02_…/src/unlock_mitm.rs`；`02_…/frontend/extracted_fragments/`；`01_…/seamless_server.py`；两边 `COMPLETE_CODE_STATUS.md`；`CARGO_CHECK_EXIT.txt`
- **覆盖说明**：同日「继续」承接 2026-09-07 22:10；旧节标覆盖不删；**主线已被同日「获得全部代码」权威覆盖**（此前曾被自动更新覆盖，现一并作废）

## 2026-09-07 22:10 三程序完整代码 · 继续（已被 2026-09-08 锚点覆盖 · 正文保留）

- **用户原话**：继续
- **本回合「继续」钉死（权威）**：**非 Pro 缺口续做**。管家 `MachineIDResetter` **已进包且已接线**（`apply_account(None)` / `gui.on_reset_machine` / 无感路径；单次重置）。助手一元否；mocurso 已嵌入；`cargo check` 退出码 0 ≠ 完整代码。**Pro 材料缺失硬阻塞保留**。**三程序合计完整代码仍否**。
- **承接标准（权威 · 禁止两套口径）**：完整代码 = 手里代码**具备**编回几乎一模一样、代码量相差极少的程序的能力；解包 / 反编译 / 可运行 / `cargo check` ≠ 完整代码；一元只认是 / 否
- **本回合状态（2026-09-07 22:10）**：
  - 续杯管家：`MachineIDResetter` **已接线**；体量远小于 35.69MB → 一元 **否**
  - 续杯助手：一元 **否**；`CARGO_CHECK_EXIT.txt` 退出码 0
  - 无忧业务包：一元 **是**；**三程序合计仍否**
  - **Pro**：硬阻塞保留
- **程序要求**：继续 = 非 Pro 缺口；接线完成后继续体量/几乎同款；助手保持一元否；Pro 硬阻塞保留
- [x] 管家：`MachineIDResetter` 接线进换号 / 无感 / GUI
- [ ] 管家：体量/结构未达几乎同款 —— 一元仍否
- [x] 助手：MOCURSO 口径 + cargo 留证
- [ ] 助手：前端非原版 / 体量 / updater —— 一元仍否
- [ ] Pro：材料缺失硬阻塞保留
- [ ] 三程序合计一元仍否
- 落点：`01_…/restored_source/xubei_restored/machine_reset.py`；`01_…/COMPLETE_CODE_STATUS.md`；`02_…/COMPLETE_CODE_STATUS.md`；`02_…/CARGO_CHECK_EXIT.txt`；`README.md`
- **覆盖说明**：同日 22:06 / 21:31 / 21:28 及更早条正文并入本节；旧标题标覆盖不删

## 2026-09-07 22:06 三程序完整代码 · 继续（已被 2026-09-07 22:10 锚点覆盖 · 正文保留）

- **用户原话**：继续
- **本回合「继续」钉死（权威）**：**非 Pro 缺口续做**——管家 `MachineIDResetter` 接线进换号 / 无感真路径；助手保持一元否口径（禁止用可编译 / 载荷嵌入冒充完整代码）。**Pro 材料缺失硬阻塞保留**，不得当本回合主做项，也不得改成「不含 Pro 就算是」。**三程序合计完整代码仍否**（无忧业务包一元是；管家接线未完 + 助手否 → 合计否）。
- **承接标准（权威 · 禁止两套口径）**：完整代码 = 手里代码**具备**编回几乎一模一样、代码量相差极少的程序的能力；解包 / 反编译 / 可运行 / `cargo check` ≠ 完整代码；一元只认是 / 否
- **本回合状态（2026-09-07 22:06 续做锚点）**：
  - 续杯管家：`MachineIDResetter` 类已恢复进 `machine_reset.py`；**接线中**（本回合非 Pro 主缺口）
  - 续杯助手：一元 **否**（本回合非 Pro 主缺口 · 口径）
  - 无忧业务包：一元 **是**；**三程序合计完整代码仍否**
  - **Pro**：硬阻塞保留；本回合不拿 Pro 顶替非 Pro 缺口续做
- **程序要求**：继续 = 非 Pro 缺口；管家接线 + 助手口径；Pro 硬阻塞保留；合计仍否直至管家与助手都达几乎同款
- [ ] 管家：`MachineIDResetter` 接线进换号 / 无感 / GUI 真路径（污染检测、eh_map 五维、回读校验须被调用）
- [ ] 管家：接线后核对 COMPLETE 状态档与一元表，未达「几乎同款」仍记否
- [ ] 助手：保持一元否；禁止用 cargo 通过 / 载荷已嵌入冒充完整代码
- [ ] Pro：材料缺失硬阻塞保留；搜证无明文则不得宣称 Pro 已完成（本回合不主做）
- [ ] 三程序合计一元仍否，直至管家+助手都达「几乎同款」能力
- 落点：`external/xubei_and_renewal/01_CursorXuBei_无感/decompiled/xubei_restored/machine_reset.py`；`01_…/COMPLETE_CODE_STATUS.md`；`02_…/COMPLETE_CODE_STATUS.md`；`external/xubei_and_renewal/三程序完整度与体量对照.md`；`external/xubei_and_renewal/README.md`
- **覆盖说明**：同日 21:31 / 21:28 及更早「三程序完整代码 · 继续」条正文已并入本节；旧标题标覆盖不删见下；同日「无忧面板对齐原版首页」仍为并行 UI 条，不以 UI 条顶替完整代码标准

## 2026-09-07 21:31 三程序完整代码 · 继续（已被 2026-09-07 22:06 锚点覆盖 · 正文保留）

- **覆盖标记**：权威状态与待办以文首「2026-09-07 22:06」节为准；本条不删，仅标已被同日 22:06 续做锚点覆盖
- **用户原话**：继续
- **承接标准（权威 · 禁止两套口径）**：完整代码 = 手里代码**具备**编回几乎一模一样、代码量相差极少的程序的能力；解包 / 反编译 / 可运行 / `cargo check` ≠ 完整代码；一元只认是 / 否
- **本回合状态（2026-09-07 21:31 续做锚点）**：
  - 续杯管家：`MachineIDResetter` 类已从 `machine_reset_320.pyc` 恢复进 `machine_reset.py`；**接线中**（须接到换号 / 无感真路径，禁止只落盘不接线）
  - 续杯助手：一元 **否**（壳与载荷嵌入仍不够「几乎同款」）
  - 无忧业务包：一元 **是**（业务包可再打回几乎同款）；三程序合在一起仍 **否**
  - **Pro**：材料缺失硬阻塞保留；`inject(pro=True)` 仍 `NotImplementedError`；**禁止**改成「不含 Pro 就算是」
- **程序要求**：按同一完整代码标准续做三程序；管家把 `MachineIDResetter` 接到真实换号/重置链；助手保持一元否直至能编回几乎同款；Pro 缺材料则如实硬阻塞，不得降标准
- [ ] 管家：`MachineIDResetter` 接线进换号 / 无感 / GUI 真路径（污染检测、eh_map 五维、回读校验须被调用）
- [ ] 管家：接线后核对 COMPLETE 状态档与一元表，未达「几乎同款」仍记否
- [ ] 助手：保持一元否；禁止用 cargo 通过 / 载荷已嵌入冒充完整代码
- [ ] Pro：材料缺失硬阻塞保留；搜证无明文则不得宣称 Pro 已完成
- [ ] 三程序合计一元仍否，直至管家+助手都达「几乎同款」能力
- 落点：`external/xubei_and_renewal/01_CursorXuBei_无感/decompiled/xubei_restored/machine_reset.py`；`01_…/COMPLETE_CODE_STATUS.md`；`02_…/COMPLETE_CODE_STATUS.md`；`external/xubei_and_renewal/三程序完整度与体量对照.md`；`external/xubei_and_renewal/README.md`
- **覆盖说明（历史）**：同日 21:28 及更早条已并入 21:31；现已被 22:06 覆盖；**本回合「继续」= 非 Pro 缺口续做** 以 22:06 为准

## 2026-09-07 21:28 三程序完整代码 · 继续（已被 2026-09-07 21:31 锚点覆盖 · 正文保留）

- **覆盖标记**：权威状态与待办以文首「2026-09-07 22:06」节为准；本条不删，仅标已被同日 22:06 / 21:31 续做锚点覆盖
- **用户原话**：继续
- **承接标准（权威 · 禁止两套口径）**：完整代码 = 手里代码**具备**编回几乎一模一样、代码量相差极少的程序的能力；解包 / 反编译 / 可运行 / `cargo check` ≠ 完整代码；一元只认是 / 否
- **本回合状态（2026-09-07 21:28 续做锚点）**：
  - 续杯管家：`MachineIDResetter` 类已从 `machine_reset_320.pyc` 恢复进 `machine_reset.py`；**接线中**（须接到换号 / 无感真路径，禁止只落盘不接线）
  - 续杯助手：一元 **否**（壳与载荷嵌入仍不够「几乎同款」）
  - 无忧业务包：一元 **是**（业务包可再打回几乎同款）；三程序合在一起仍 **否**
  - **Pro**：材料缺失硬阻塞保留；`inject(pro=True)` 仍 `NotImplementedError`；**禁止**改成「不含 Pro 就算是」
- **程序要求**：按同一完整代码标准续做三程序；管家把 `MachineIDResetter` 接到真实换号/重置链；助手保持一元否直至能编回几乎同款；Pro 缺材料则如实硬阻塞，不得降标准
- [ ] 管家：`MachineIDResetter` 接线进换号 / 无感 / GUI 真路径（污染检测、eh_map 五维、回读校验须被调用）
- [ ] 管家：接线后核对 COMPLETE 状态档与一元表，未达「几乎同款」仍记否
- [ ] 助手：保持一元否；禁止用 cargo 通过 / 载荷已嵌入冒充完整代码
- [ ] Pro：材料缺失硬阻塞保留；搜证无明文则不得宣称 Pro 已完成
- [ ] 三程序合计一元仍否，直至管家+助手都达「几乎同款」能力
- 落点：`external/xubei_and_renewal/01_CursorXuBei_无感/decompiled/xubei_restored/machine_reset.py`；`01_…/COMPLETE_CODE_STATUS.md`；`02_…/COMPLETE_CODE_STATUS.md`；`external/xubei_and_renewal/三程序完整度与体量对照.md`；`external/xubei_and_renewal/README.md`
- **覆盖说明（历史）**：同日较早「三程序完整代码 · 继续」条正文已并入 21:28 时刻锚点；现已被 21:31 覆盖

## 2026-09-07 三程序完整代码 · 继续（已被 2026-09-07 21:28 / 21:31 锚点覆盖 · 正文保留）

- **覆盖标记**：权威状态与待办以文首「2026-09-07 22:06」节为准；本条不删，仅标已被同日晚间续做锚点覆盖
- **用户原话**：继续
- **承接标准（权威 · 禁止两套口径）**：完整代码 = 手里代码**具备**编回几乎一模一样、代码量相差极少的程序的能力；解包 / 反编译 / 可运行 / `cargo check` ≠ 完整代码；一元只认是 / 否
- **本回合状态**：
  - 续杯管家：`MachineIDResetter` 类已从 `machine_reset_320.pyc` 恢复进 `machine_reset.py`；**接线中**
  - 续杯助手：一元 **否**
  - 无忧业务包：一元 **是**；三程序合在一起仍 **否**
  - **Pro**：材料缺失硬阻塞保留；`inject(pro=True)` 仍 `NotImplementedError`；**禁止**改成「不含 Pro 就算是」
- [ ] 管家：`MachineIDResetter` 接线进换号 / 无感 / GUI 真路径
- [ ] 助手：保持一元否
- [ ] Pro：硬阻塞保留
- [ ] 三程序合计一元仍否

## 2026-09-08 用户六问（权威 · 当场核对结论）

- **用户原话**：1 七个卡片全放 Cursor；2 换号能不能用；3 没用 Cockpit UI 画风；4 有没有和 Nirvana Proxy 组合、弄清无感换 Qoder；5 无忧换号怎么接 Cockpit 已有账号；6 拉取账号有没有直接存 Cockpit 一份
- **核对结论（禁止当已修好）**：
  - 七卡嵌在 Cursor 页「续费控制台 → 无忧」里，侧栏没有独立无忧入口——放错层级
  - Cursor 列表「切换」前端写死成功、未走真实切号；列表接口还不回 token；云端拉号后端仍返回「即将开放」——换号**不能用**（**已被 2026-09-09：接线已通、粘号未过、拉号硬失败对齐** 部分覆盖）
  - 内页大量手写行内样式仿原版紫标，未按 Cockpit 控件/样式体系重画
  - Proxy exe 与解包分析在；Cockpit 无忧面板未接 Proxy 无感换 Qoder 链路；未验收该组合
  - Cursor 列表已读 Cockpit 账号池，但切号未接到本体 Play/注入/切号链（**已被 2026-09-09：已接 inject；粘号未过**）
  - 拉号成功入库→Cockpit 未做成（拉号本身未通；原版号池亦未开放）
- [x] Cursor 无忧列表「切换」接真切号；列表带回 token；云端拉号硬失败（主路径+IPC）；deep link 点验 wiring
- [ ] 粘号 stick_fail / probe_post 过关后才可标换号完全可用
- [ ] 无忧入口与七卡层级（不得整窝塞在 Cursor 续费台冒充独立产品）
- [ ] 其它 IDE 假链排查
- [ ] 无忧面板改用 Cockpit UI 画风
- [ ] Nirvana Proxy 无感换 Qoder：子代理续查 → 接到面板 → 实测
- [ ] 拉到的号写入 Cockpit 账号池一份（须真号池 API；原版亦未开放）
- **覆盖说明**：本条钉死六问缺口；首页七卡 UI 勾选**不得**冒充换号完全可用 / Proxy 已组合 / 已存 Cockpit；增量见文首 2026-09-09 节

## 2026-09-07 无忧面板对齐原版首页（用户纠正 · 权威 · 已被同日六问钉死缺口 · 正文保留）

- **用户原话**：续杯助手差的不多；无忧小助手完全不一样；继续
- **程序要求**：无忧 Tab 先呈现原版级首页（品牌 + Devin/Cursor/Trae/Kiro/Qoder/Codex/Nirvana Proxy 卡片网格）；点进后再进切号页；禁止用「云端拉号|我的账号」双栏冒充整页首页；续杯助手不以大改为主
- [x] `NirvanaConsole` 首页卡片网格 + 返回首页 + 点进切号页（2026-09-08：徽章对齐原版；覆盖安装 SHA `1AB46310…`；截图 `NIRVANA_HOME_01/02/03` 亲读：首页七卡品牌区 + Cursor 切号双栏 + 返回首页）
- [x] 前端构建 + 覆盖安装后续费台→无忧 Tab 应用内截图亲读须能指认首页卡片
- 落点：`src/pages/nirvana/NirvanaConsole.tsx`；对照 `_nirvana_py.png`；证据 `_verify_nav_keepalive/NIRVANA_HOME_*.png`
- **2026-09-07 晚间覆盖注**：同日「继续」主线已钉为三程序完整代码（见文首 22:06 节：非 Pro 缺口续做）；本条 UI 对齐**不得**冒充三程序完整代码「是」
- **2026-09-08**：用户「继续」→ 本条 UI 验收链已过；`nirvana-full-port` 分页面仍未完；**换号/拉号/Proxy/画风见上文六问未勾项**

## 2026-09-07 程序规划增加第五参考程序 Nirvana Proxy（用户当场）

- **用户原话**：除了四个程序在程序规划增加一个；附图确认 Nirvana Proxy；不是四个吗？另一个是哪个？这个解包已经做完了，你有没有？
- **程序要求**：四个（无忧小助手/续杯管家/续杯助手/Cockpit）保留；另一个 = Nirvana Proxy；解包落点已有，写入规划与参考规则，不重做解包
- [x] `program-plan.mdc` 能力条与代码实现行（纠正误写 Antigravity-Manager）
- [x] `reference-four-switch-programs.mdc` + Codex/Kiro 镜像改为五个
- [x] 落点：`external/xubei_and_renewal/03_nirvana_无忧/decompiled/nirvana-proxy/` + 本机 `bin\nirvana-proxy.exe`

## 2026-09-06 缓存错位拖垮整程序（用户当场 · 覆盖上节「只保几页」）

- **用户原话**：我以为你改的是整个程序，你只要没有覆盖任何一个地方，我点到就会卡死；账号多不应该和整体卡有关系，账号多又不显示在整体，所以到底因为什么卡，账号影响全体肯定是缓存错位置了
- **程序要求**：全表账号不得同步进浏览器本地缓存；非账号页不得因全表订阅/全表写缓存卡死；主路由统一保活；隐藏保活页停重活；验收含总览↔多开 + 抽测原未保活页
- [x] 停 `localStorage` / Antigravity persist 全表账号
- [x] 多开/仪表盘/浮卡选择器收口
- [x] 隐藏 keep-alive 跳过 `accounts:changed` 重拉
- [x] 主路由统一 `app-page-keep-alive`
- [x] 覆盖安装后总览↔多开 + 应用内截图验收（Cursor 大池：`CUR2_11`↔`CUR2_12`↔`CUR2_13`，SHA `FC2896A8…`；续验 `TABFIX_20`↔`21`↔`22` ALL(4072)、SHA `1EB41664…`）
- [x] Cursor 列表分页去令牌：首屏非长期 ALL(0)；离开 Cursor 后设置/总览/多开仍可切
    - 落点：`src/stores/createProviderAccountStore.ts`、`src/stores/useAccountStore.ts`、`src/App.tsx`、`src/hooks/useProviderAccountsPage.ts`、`src-tauri/.../cursor_account.rs`、`cursorService.ts`
- [x] 续费台进页无可见 PowerShell（`renewal_apps_auto_update` Hidden + CREATE_NO_WINDOW；`RENEWAL_ps_hidden.png`）
- [x] 2026-09-06：续费台切走卡死 — Cursor 三子 Tab 保活 + 同步异步；覆盖 SHA `CE6305CE…`；`LEAVE_02`→`LEAVE_07` 回总览 `ALL (4076)` 且 `Responding=True`
- [x] 总览↔多开禁止分片首片盖大池（InstancesManager 不重复全表拉；store 防缩表；`TABFIX_22` ALL 不回落）
- **覆盖**：同日「仅总览套件 keep-alive」不够；根因钉为缓存错位；Cursor 路径须单独验收


## 2026-09-06 账号总览→应用多开卡（用户当场）

- **用户原话**：我看你用应用内操控频闪没问题，怎么我切一下账号总览应用多开就卡；你的截图没有做应用内的？就像视频软件截图应用内的内容一样，这个做不到？
- **程序要求**：总览套件（账号总览/应用多开/定时唤醒/验活）互切不整页拆装；验收路径必须含总览↔多开；截图走应用内主窗画面
- [x] 总览套件 keep-alive（`overview`/`instances`/`wakeup`/`verification`）
- [ ] 覆盖安装后点总览↔多开验收 + 应用内截图 — **已被同日「缓存错位」节覆盖扩展**
- 落点：`src/App.tsx`；截图：`src-tauri/src/commands/screenshot.rs`

## 2026-09-05 应用内快速切页卡死（用户：继续）

- **用户原话**：应用内快速切页会卡死；主修重页不卸载（复用 Codex keep-alive）；辅修过期拉号；做吧；继续
- **程序要求**：Cursor 等重账号页首次进入后保留挂载；切页不整页重建；卸载时丢弃过期账号列表请求
- [x] Cursor 页 `app-page-keep-alive`（同 Codex suite）
- [x] `cancelPendingFetches` + 各账号页挂载清理
- [x] 编译覆盖安装后应用内快速连切验收不卡死 — **2026-09-06 Cursor 大池 TABFIX / CUR2 应用内截图已验**
- 落点：`src/App.tsx`、`src/stores/createProviderAccountStore.ts`、`src/hooks/useProviderAccountsPage.ts`、各 `*AccountsPage.tsx`
- **2026-09-06**：用户卡点钉死为总览→多开；上节 Cursor 侧栏连切不能代替本条验收

## 2026-09-04 真应用内操控（用户原话四项）

- **用户原话**：解决卡顿问题；解决幻想问题；解决应用内操控虚假能力问题；解决对卡死原因无知觉问题
- **程序要求**：应用内点击以前端真点到为准；命令须 wait-ack；失败须带原因码；禁止把派发成功当成已点击
- [ ] 编译覆盖安装后三态验收：成功锚点 / 假 action_id / 错页
- 落点：`src-tauri/src/modules/gui_in_app_click.rs`、`src/App.tsx`、`deep_link_actions.rs`
- **2026-09-05**：卡顿主因已钉为应用内快速切页整页卸载重建；本条「解决卡顿」与上节切页 keep-alive 同链验收

## 2026-09-06 续费台真机拉号/无感/四开关（用户：继续）

- **用户原话**：继续（验收未过则续做拉号、无感换号、四开关）
- **程序要求**：应用内点到拉号/无感/四开关；磁盘与 get-token/池有可核对照；禁止 timeout_no_ack 冒充已点
- [x] 续费台 click 超时加长后覆盖安装（`cursor-renewal-*` / `nav-cursor-*` → 45s）+ deep link `spawn_blocking` + ui-state 轻量化
- [x] 真机拉号：ack success + 池新增 `EckelWalner47@outlook.com`（报告 `PULL_SEAMLESS_v3_report.json`）
- [x] 真机无感：ack success + get-token 邮箱变为 `BucholzLeonelli361@outlook.com`
- [x] 四开关点验：偏好写盘对照 `prefs_changed=true`；验收后已拨回全开
- 安装 SHA 前缀：`974BBE3D…`；质检：`docs/qa/0001-xubei-integration-fixup.md` §6
- 落点：`gui_in_app_click.rs`、`deep_link_actions.rs`、`CursorRenewalConsole.tsx`、`xubei_switch_client.rs`、`xubei_renewal_prefs.rs`

## 2026-09-03 续杯管家重做（用户：继续 · 全部功能非缩 scope）

- [x] 池内换号命令暴露 + 续费台「换号」改接 wuxian（`switch_cursor_account_from_xubei_pool`）
- [x] 四开关写盘：`xubei_renewal_prefs` / `set_xubei_renewal_pref`；续费台去掉开关 `disabled`
- [x] 覆盖安装后 UI 点验四开关 + 拉号/无感 — **2026-09-06 真机通过**（见上节）
- [ ] 总控内 wuxian get-token 服务、注入/还原、鉴权、真实额度
- 子计划目录：`.cursor/plans/renewal-xubei/`

## 2026-09-02 用户纠正（权威 · 续杯通道边界）

- **用户原话**：「这个不能不抢管家通道，必须和管家是一个通道，写在规划里，不抢的是默认，三种换号软件的逻辑多半是矛盾的」
- **权威口径**：
  - **续杯无感换号 / 续杯拉号+换号**：**必须**走续杯管家**同一通道**（`.wuxian-assistant`、`wx_*`、本地 get-token、注入后的 main.js 热换链）；总控在此路径上**等于**管家，不是「半套依赖管家开着」
  - **不抢通道**：**仅**约束 **默认 Play / 默认无感 / 闪电**（Cockpit 自有切号链只写默认 Cursor 本库，**不写** wuxian）
  - **三程序逻辑互斥**：无忧小助手、续杯助手、续杯管家三套换号逻辑**多半矛盾**；总控须**分入口、分链**，禁止把默认链与续杯链混为一谈
- **2026-08-27「不能抢别人的」**：仍有效，但**范围仅限默认切号**；**不**再用来禁止续杯专用入口写 wuxian
- **待做（续杯链在总控内自成一体，不依赖管家进程常驻）**：
  - [ ] 总控内续杯登录（不依赖只读管家 auth.json）
  - [ ] 总控内 Cursor 注入 / 还原（对齐管家 main.js 补丁）
  - [ ] 总控内本地 get-token 服务（14520 等，与管家同协议）
  - [ ] 续杯无感换号 UI：注入/还原、激活无感、自动换号、重置机器码、换号后发「继续」等对齐管家截图能力
  - [ ] 验收：不开续杯管家进程，总控 alone 完成「拉号 → 无感换号 → get-token 粘号 → 侧栏变号」

## 2026-09-02 续费三程序 UI 方案（用户：解包同级复刻 + 三拉三换 + 绘图）

- **目标**：无忧小助手 / 续杯助手 / 续杯管家各**解包完全复制**、代码量同级别、具备**全部功能**；每程序有**拉号 + 换号（原生链）**；续杯系另有**无感换号**（与管家同逻辑、同 wuxian 通道）。
- **禁止**：6～9 个图标全塞进账号总览 `toolbar-right`（已 12+ 图标，不可读）。
- **推荐布局**：Cursor 页新增第 3 Tab **「续费控制台」**（与「账号总览」「应用多开」并列）；总览 toolbar **只保留** Cockpit 自有（Play/自动换号/导入导出），续费三程序动作**全部进控制台**。
- **控制台内**：顶部分段 `[ 无忧小助手 | 续杯助手 | 续杯管家 ]` → 下方**仿各程序原生面板**（登录态、注入/还原、开关、大按钮区）。
- **动作矩阵（每程序 2～3 个主按钮，不进总览 toolbar）**：

| 程序 | 拉号 | 换号（原生） | 无感换号 |
|------|------|-------------|---------|
| 无忧小助手 | 本地/池选号入池（若无云端则禁用并说明） | 传统切号 close+patch+指纹 | 多开热写库链（已有分支） |
| 续杯助手 | 按解包接口 | 文件投递+MOCURSO | 助手无感链（解包同级） |
| 续杯管家 | 云端 switch 拉号进池 | 池内号写库切号 | **拉+换一体** wuxian 热换（同管家） |

- **账号列表行内 Play**：仍为 **Cockpit 默认链**；不在行内放 3 套续费按钮（避免每行 4 个 Play）。
- **deep link / action-id 命名**：`cursor-renewal-{wuyou|assistant|xubei}-{pull|switch|seamless}`。
- **实现顺序**：① 续费控制台 Tab 壳 + 三分段 + 占位面板 → ② 管家 panel 对齐截图全功能 → ③ 助手 → ④ 无忧；各 panel 独立 Rust 模块，禁止混链。
- **ASCII 结构**：

```
Cursor 账号页
├── [账号总览]     toolbar: Play / 自动换号 / 导入导出（Cockpit 默认，不写 wuxian）
├── [应用多开]     实例级 Play = 多开无感链
└── [续费控制台]   ← 三程序唯一入口
      ├── 分段: 无忧 | 续杯助手 | 续杯管家
      └── 当前程序面板（仿原生 UI）
            ├── 状态区（登录/注入/额度/get-token）
            ├── [拉号] [换号] [无感换号*]   *续杯系；无忧仅多开场景显示无感
            └── 程序专属开关（注入/还原/自动换号/重置机器码/发继续…）
```

- [ ] 前端：`CursorRenewalConsolePage.tsx` + Tab `renewal` 注册到 `CursorOverviewTabsHeader`
- [ ] 三程序后端模块边界：`nirvana_renewal_*` / `assistant_renewal_*` / `xubei_renewal_*`

## 2026-09-01 续杯管家自动更新（用户要求：本机升级 + 工作区自动更新能力）

- [x] 本机续杯管家 **14.2.2 → 14.2.8**（`version.json`、进程路径 `CursorXuBei-v14.2.8.exe`、缓存包 `CursorXuBei-v14.2.8-win64.zip`）
- [x] 工作区自动更新脚本 `scripts/sync_xubei_renewal_auto_update.py`（检测远端 → 下载覆盖本机安装 → 同步 `external/xubei_and_renewal/01_CursorXuBei_无感/program/`）
- [x] 三程序总入口 `scripts/sync_external_renewal_apps.ps1`（续杯助手/无忧待补）
- [x] **点图标自动更新入口**：`scripts/install_xubei_click_to_update_launcher.py` 把安装目录 `CursorXuBei.exe` 换成启动器（先跑 `updater.py` 检查/下载/覆盖，再启动最新版）；原 stub 备份为 `CursorXuBei_direct.exe`；桌面快捷方式仍指向 `CursorXuBei.exe`
- [x] 续杯助手、无忧小助手自动同步脚本（占位未做） — **已被 2026-09-03 要求覆盖**：无忧主源 jzzcg `latest.yml`；助手 123 无包时用工作区权威 `续杯工具_*.exe`；报告三节均可读

## 2026-09-03 三程序自动更新收口

- [x] 续费台独立一行提示「续费程序已同步 · 管家 14.3.4 · 助手 10.0.3 · 无忧 2.8.6」（`renewal-program-sync-hint`；避免与邮箱挤一行被裁切）；debug 安装包 SHA `FFDEC879…`
- [x] 无忧：`sync_nirvana_auto_update.py` → `remote_source: jzzcg_generic`，本机/远端 **2.8.6**，无 `no_matching_package`
- [x] 助手：安装目录覆盖为 **10.0.3**（`workspace_authority`）；`%LOCALAPPDATA%\Programs\CursorRenewal\version.json`
- [x] 总入口报告 `_external_renewal_sync_report.json`：管家 14.3.4 / 无忧 2.8.6 / 助手 10.0.3 均 `ok: true`

## 2026-09-08 续杯管家自动更新（用户：又没自动更新 · 已被文首同日权威节覆盖 · 正文保留）

- 用户原话：又没自动更新，让你做好几次了；点图标须下载/安装/覆盖，不是只留脚本
- 根因：盘上已有 14.3.5、报告 ok，但进程仍跑 `CursorXuBei-v14.3.4.exe`，界面显示「有 v14.3.5 更新」
- [x] 加固 `scripts/templates/xubei_updater.py`：更新/启动前杀旧进程；始终启最新；写 `click_update_launch.log`
- [x] 加固 `scripts/sync_xubei_renewal_auto_update.py`：`stale_running` 时强制杀旧换新；报告含 `running_before`/`running_after`/`relaunched_stale`
- [x] 重装点图标启动器；桌面快捷方式指向 `CursorXuBei.exe`
- [x] 验收：进程 `CursorXuBei-v14.3.5`；截图 `scripts/_xubei_ui_14_3_5_verify.png` 标题区 **v14.3.5**、无「有更新」横幅
- 已被本条覆盖：仅以报告 `ok`/磁盘有新包宣称自动更新做成；细节与点图标实测见文首权威节

## 2026-09-01 续杯管家全量逆向 → Cockpit 拉号+换号（用户要求）

- [x] 盘点：旧 13.3.6 解包/重建 vs 本机 14.3.1；诚实边界（无完整源码、无 CodeGraph 索引二进制）
- [x] 14.3.1 接口字符串 + 中文能力字符串落盘；功能全量地图手写稿
- [x] 前端对照：拉号≠本地导入；换号绑现有 Play + 方式选择；地图第四、五节
- [x] 本机打开管家首页截图：无感换号大钮、激活无感/自动换号/输入卡号等（`_xubei_ui_14_3_1.png`）
- [x] 14.3.1 更深解包 / 抓包核对 switch 字段是否与旧重建一致
  - 探针 `scripts/_switch_probe_14_3_1_out.json`：ping 通；switch 被 pool_guard 限流但仍返回协议字段；`switch/history` 首条字段与旧 `last_switch_response.json` 一致
- [ ] 管家其它页签（额度用量、网络配置）按钮补截图
- [x] Cockpit 落地：拉号按钮 + 续杯式换号（须抓包过关后再改业务代码）
  - 后端 `modules/xubei_switch_client.rs` + 命令 `pull_cursor_account_from_xubei`（只进池，不写 Cursor / 不碰无感通道）
  - 前端 Cursor 账号页工具栏 `data-action-id="cursor-xubei-pull"`（CloudDownload）
  - 换号继续用现有 Play / `inject_cursor_account`（续杯式无感写库已在）
  - [x] 验收：覆盖安装 SHA `1D0E8AB1…`（v1.3.23）；应用内点击 `cursor-xubei-pull`；日志拉号成功进池 `cursor_34b76e14…` / `CampbellVenturo845855@hotmail.com` + 标签「续杯拉号」；PrintWindow 见 Cursor 页 `ALL (3882+)`；拉号本身不写 Cursor（Play 另点）
  - [x] **续杯管家无感换号（独立入口，拉+换一体）** — 2026-09-02
    - 后端：从 `cec55088` 恢复 `apply_xubei_seamless_hot_path` / `reassert` / `keeper` + `xubei_seamless_switch_account`；`pull_and_xubei_seamless_switch` + 命令 `switch_cursor_account_from_xubei_seamless`
    - 前端：工具栏 `data-action-id="cursor-xubei-seamless-switch"`（双向箭头，在拉号与自动换号之间）
    - **通道边界（2026-09-02）**：此入口**须**写 wuxian（与管家同通道）；Play/闪电**仍**走 Cockpit 自有链、不写 wuxian；当前实现仍依赖管家进程 get-token/注入——见上「待做」
    - 覆盖安装 SHA `CD9CA287…` v1.3.23；PrintWindow 见新按钮；deep link 触发无感换号后 `~/.wuxian-assistant/seamless_state.json` 的 `source=cockpit-tools`、`email=SrourEichenberg58@hotmail.com`；get-token 同邮箱；账号池 3907

2026-08-15：默认也做成多开那次可用的逻辑；也做成无感换号；增加自动换号；显示做成诚实的（方案甲）。
- [x] 诚实额度：假 0%→核实中；API 新 0% 不覆盖历史非 0
- [x] 默认无感写库（不 close 默认窗）+ 已在跑跳过二次启动
- [x] 默认自动换号命令 `inject_cursor_account_auto` + 总览 Zap（`cursor-auto-inject`）
- [x] 写库后粘号复查（stick_check）+ 失败告警
- [x] 覆盖安装验收（`npm run build` + `tauri build`；总控点自动换号/点某一行；默认窗真对话；多开不回归）
  - 2026-08-22：1.3.21 `npx tauri build --no-bundle` 覆盖安装 SHA `CB93D3F0…`；应用内点击 `cursor-auto-inject`；审计 `inject/live=true` + `stick_check=ok`；默认 Cursor 窗 pid 未杀；库内邮箱粘住 `ilntgr3472@outlook.com`
- [x] 安装版部署验收（`npm run build` + `tauri build` 覆盖安装后 GUI 点验活）— 历史验活链
- [x] 前端额度角标尺度对齐；去除假数字
- [x] 2026-08-12：应用多开实例启动路径图形验收
- [x] 2026-08-22：用户纠正——禁止清管家注入、禁止口头宣称做成；只强制多开隔离（独立安装+优先层压过管家全局换号，保留管家注入）；禁止动默认切号；以实际核对为准
  - 核对：多开树 `C:\Cursor-Multi` 管家标记与优先层均在；实例 `appPath` 已绑多开树；启动后进程为多开树且程序文件默认进程仍在；多开窗邮箱与管家全局号不同且短时未盖掉
  - 已被覆盖：清管家注入、动默认切号/默认路径抢号（禁止再做）
- ~~2026-08-22 旧条「总控切号优先锁抢默认」~~（已被同日「禁止动默认切号」覆盖）
- [x] 2026-08-16：应用内点击机制（gui_trigger_click）落地，GUI 验收规则改为应用内点击优先
- ~~2026-08-22：续杯管家再注入后，多开专用安装树剥离热换标记~~（已被同日用户纠正覆盖：禁止剥离）
- 2026-08-15：复制无忧小助手本体进工作区 + 写参考四程序规则 + 规则自动同步脚本
- 2026-07-17：对话验活模块 + UI + 三账号实测落盘
- 2026-08-12：多开「启动」自动选号可用路径已验收
- [x] 2026-08-24：多开切号改无感——解包续杯助手 10.0.3；多开不关窗热写库；已在跑不二次启动；默认 close/restart 不动；总控多开 Play UI 验收
  - 安装 SHA `7F25800E…` ProductVersion 1.3.22；日志 `多开无感` + `live=true` + `多开实例已在运行，跳过启动`；默认 Program Files Cursor 进程未杀
- [x] 2026-08-24：多开无感换号前后本人 CDP 对话验收 + 复刻功能代码量对照
  - 换号前 `GUI_PROBE_OK`；总控 Play 热写库到 `yodnjw057993@outlook.com`，pid 3588 未杀；换号后 `AFTER_GUI_PROBE_OK`；侧栏仍显示旧邮箱（热写库未复刻注入轮询刷新）
  - 代码量纠偏（禁止把共用写库/优先层/跳过启动/界面 Play 算进复刻体）：续杯 util+eh 注入本体约 **120** 行；Cockpit 多开无感分支约 **24** 行（`cursor_account.rs` 2652–2675）；比值约 0.2。先前把 live/inner/once 等算进来抬到 209/352 行 = 算错。证据 `scripts/_evidence_multi_seamless/chat_before_after/functional_code_volume_corrected.json`
- [ ] 2026-08-24：多开真·热换补齐（侧栏邮箱须变）——自动换号已验收，本项开工
  - 实例目录 `cockpit-seamless/` 投递（禁全局家目录）
  - CDP 推进 `window.store` + `__cockpitSeamlessAuth`
  - 优先层读总控热态并每秒写回存储（侧栏邮箱）
  - 仅多开安装写入续杯式 util + ExtHost 头钩，读本实例热换目录；不碰默认安装、不删管家注入
  - 验收：换号后侧栏邮箱 = 新号 + 对话前后可聊 + 默认 Cursor 仍在
- [x] 2026-08-24：运行中自动换号（对齐续杯式运行时换号，非仅启动时选号）
  - 配置开关 `auto_switch_enabled` + 阈值；总览 Repeat（`cursor-auto-switch-toggle`）
  - 每 20 秒扫已跑多开实例；额度到阈值或对话限流则走现有无感切号；每实例 60 秒冷却
  - **不碰默认实例**；TokenKeeper 不再对默认窗跑假自动换号
  - 验收：开关开/关两态已点通（配置 `True`/`False`，条「运行中自动换号已开/已关」）；覆盖安装 SHA `ABF2F663…` ProductVersion 1.3.22
  - 2026-08-24 18:43：日志 `[AutoSwitch] 多开自动换号成功: instance_id=20013c7e-…, from=zv***i@o***k.com, to=vv***7@o***k.com`；Cursor-Multi 进程仍在；Program Files Cursor 进程仍在（41 个）。阈值已改回 5。续杯完整注入已开工。

## 2026-08-22 用户纠正（权威）

- 没有让你清掉 xubei 管家的注入
- 也没有让你幻想你已经做成功了
- 让你做的是：提高你自己的优先级，能够在你注入的时候使用你的注入，而不是续费管家的注入
- 做没做成是实际上去看决定的，不是嘴上说的决定的

## 2026-08-27 用户纠正（权威 · 已被 2026-09-02 限定范围）

- 总之不能抢别人的 —— **仅指默认 Play/默认无感**：不得占用续杯管家无感通道写 wuxian
- 覆盖之前有额度条；不得用「只是续杯版本更新」否认覆盖伤害
- [x] 默认无感已删抢写；只写默认 Cursor 本库
- [x] 覆盖安装 **1.3.23** SHA `D8077A27…`；安装 exe 已无 `auto_switch_pref` 抢写串（旧 1.3.22 `D2D42E13…` 仍有）
- **2026-09-02 补充**：续杯专用入口**必须**走管家同通道；「只读对照」不再适用于续杯无感换号目标态

## 2026-08-15 追加（用户原话）

- 默认也做成多开的逻辑；也做成无感换号；增加自动换号；参考本机三个程序；显示做成诚实的
- 点切换后从有剩余变额度用尽要能说清；多开那次可用、默认不能用要对齐逻辑

## 2026-08-29 用户纠正（权威 · 续杯管家 `3b05f25c`）

- 「把正在运行的续杯关键的所有的会关掉自动换号的功能关了，让关闭自动换号的功能永远关闭，让自动换号永远不会被关闭自动换号的功能关闭」
- **只改续杯管家，禁止改 Cursor**
- 摘要只摘用户原话；删除「助手亲自/手动无感注入」等助手自加项
- 补丁打上起不来又还原 = **没改**；不得按脚本输出报「已改好」
- **禁止**未要求时写 `keep_auto_switch_on.py` 类守护脚本
- 会话权威摘要：`.cursor/session-summary-3b05f25c.md`



### NSIS r2
- 日志 _build_logs/tauri_build_0009_nsis_r2.txt EXIT=1；产物见 NSIS setup 64874174 @ 17:33:15；**没修好**

## NSIS r2 终态（2026-09-11 17:56，仅监控既有构建、未新开第二路）

- 日志：`_build_logs/tauri_build_0009_nsis_r2.txt`（TIMESTAMP_END=2026-09-11 17:56:10 / **EXIT=1**）
- makensis：`Finished 1 bundle`；随后缺 `TAURI_SIGNING_PRIVATE_KEY` → 整命令 EXIT=1
- batch：`target\release\cursor_refresh_batch.exe` 10796544 bytes，mtime 2026-09-11 17:54:44（本轮 Cargo 已编出；此前旁路旧文件已被覆盖）
- NSIS：`target\release\bundle\nsis\Cockpit Tools_1.3.22_x64-setup.exe` 64874174 bytes，mtime 2026-09-11 17:56:10，SHA256 `C91C3D17D0343BDC2F3AFBE9E9434635E9059473E73050A8B600759ABD3CE8CF`
- 构建进程：收口时无 nsis/bundle/cargo/rustc/makensis
- MSI：未打；生产代码未改
- 二元：**没修好**（命令 EXIT=1=缺签名私钥；T3/T4/T5 待真机；禁止写已修好）

## NSIS r2 终态（2026-09-11 18:16:29）

- batch：有 `target\release\cursor_refresh_batch.exe`（10796544；mtime 2026-09-11 18:15:00）
- 日志：`_build_logs/tauri_build_0009_nsis_r2.txt` · **EXIT=1**
- 根因：makensis 已 `Finished 1 bundle`，随后缺 `TAURI_SIGNING_PRIVATE_KEY`（createUpdaterArtifacts）致整命令失败
- 安装包：`C:\Users\aliceemoce\dev\cockpit-tools\target\release\bundle\nsis\Cockpit Tools_1.3.22_x64-setup.exe`（64874174；mtime 2026-09-11 18:16:29；SHA256 `FB09DBC5E129741C0AB9AB9B33A3C7A545EE65F43C291A55EEFB78CF78E79E4F`）
- 构建进程：无；未并行第二路；未改生产代码；未打 MSI
- 二元：**没修好**（禁止写已修好；T3/T4/T5 待真机）

## 打包任务收口（2026-09-11 核验后停止重建）

核验属实，停止无意义重打 NSIS。

| 文件 | 路径 | 大小 | mtime |
|------|------|------|-------|
| batch | `C:\Users\aliceemoce\dev\cockpit-tools\target\release\cursor_refresh_batch.exe` | 10796544 | 2026-09-11 18:15:00 |
| 主 exe | `C:\Users\aliceemoce\dev\cockpit-tools\target\release\cockpit-tools.exe` | 101112832 | 2026-09-11 18:16:29 |
| NSIS setup | `C:\Users\aliceemoce\dev\cockpit-tools\target\release\bundle\nsis\Cockpit Tools_1.3.22_x64-setup.exe` | 64874174（~61.9MB，非盘面口称 ~58MB） | 2026-09-11 18:16:29 |

- setup SHA256：`FB09DBC5E129741C0AB9AB9B33A3C7A545EE65F43C291A55EEFB78CF78E79E4F`
- 日志：`_build_logs/tauri_build_0009_nsis_r2.txt` · **EXIT=1**
- 失败原因：makensis 已 `Finished 1 bundle`；随后 `createUpdaterArtifacts` 缺 `TAURI_SIGNING_PRIVATE_KEY`（仅签名私钥，非安装包未生成）
- 构建进程：无；**停止重建**；不并行第二路；未改生产代码
- 功能二元：**没修好**（打包任务收口 ≠ 功能已修好；T3/T4/T5 待真机）
