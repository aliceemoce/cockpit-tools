# 参考五个程序 · 出问题从不出问题的地方找或复刻

## 用户原话（权威）

- 「制作程序的时候，需要参考这三个续费软件和cockpit的主仓库本体」
- 「你的代码是要参考这三个程序和cockpit的本体的主仓库如果出了问题，就从这几个程序不出问题的地方找哪里问有问题，或者直接复刻代码」
- 「这些东西也要做成自动更新。自动安装，自动覆盖」
- 「无忧小助手的换号方式不能删除」
- 「除了四个程序在程序规划增加一个，你应该知道哪个把」（2026-09-07；附图确认 **Nirvana Proxy**）
- 「不是四个吗？另一个是哪个？」「这个解包已经做完了，你有没有？」（2026-09-07）

## 五个参考程序落点（可点引用）

| 程序 | 工作区落点 | 说明 |
|---|---|---|
| 无忧小助手（nirvana） | [external/xubei_and_renewal/03_nirvana_无忧/](file:///c:/Users/aliceemoce/dev/cockpit-tools/external/xubei_and_renewal/03_nirvana_无忧/) | Electron 应用，jzzcg-ai-suite；切号逻辑已对齐进 [cursor_switch_align.rs](file:///c:/Users/aliceemoce/dev/cockpit-tools/src-tauri/src/modules/cursor_switch_align.rs)（从它的 `main-FXxcqbQA.js` 解出的 `i()` 函数 @20973 行） |
| CursorXuBei（虚备/续杯管家） | [external/xubei_and_renewal/01_CursorXuBei_无感/](file:///c:/Users/aliceemoce/dev/cockpit-tools/external/xubei_and_renewal/01_CursorXuBei_无感/) | Nuitka standalone，本地 wuxian-assistant HTTP 服务热换 token |
| 续杯助手（cursor-renewal） | [external/xubei_and_renewal/02_续杯助手_无感/](file:///c:/Users/aliceemoce/dev/cockpit-tools/external/xubei_and_renewal/02_续杯助手_无感/) | Tauri + WebView2，文件投递 + MOCURSO 补丁 |
| Cockpit 主仓库本体 | 当前工作区 `c:\Users\aliceemoce\dev\cockpit-tools` | 即本项目 |
| Nirvana Proxy | [external/xubei_and_renewal/03_nirvana_无忧/decompiled/nirvana-proxy/](file:///c:/Users/aliceemoce/dev/cockpit-tools/external/xubei_and_renewal/03_nirvana_无忧/decompiled/nirvana-proxy/) | 与无忧小助手分列的本机代理（Wails/Go）；解包已有（`source/*.go` + 目录内 exe）；本机运行态 `%USERPROFILE%\.antigravity_cockpit\bin\nirvana-proxy.exe` |

## 硬要求

1. **制作程序时必须参考这五个程序**：写切号、换号、无感、额度显示、自动选号、代理链路等逻辑前，先看这五个程序是怎么做的。
2. **出问题从不出问题的地方找**：Cockpit 某条链路不通时，先去看这五个程序里同一功能不出问题的地方是怎么实现的，找到差异点或直接复刻代码。
3. **直接复刻代码**：如果某个程序的做法已验证可用，Cockpit 可以直接复刻它的代码路径（注意技术栈差异：Python/Rust/JS/Go 转换）。
4. **无忧小助手切号方式不能删除**：[cursor_switch_align.rs](file:///c:/Users/aliceemoce/dev/cockpit-tools/src-tauri/src/modules/cursor_switch_align.rs) 里已对齐的无忧式传统切号路径（close → switchTokensInDb → resetStorageJsonIds → resetMachineIdFile → patchCursorMachineId → resetWindowsMachineGuid → 启动前等 1.5s）必须保留。
5. **五个程序都要做成自动更新/自动安装/自动覆盖**：
   - Cockpit 本体：见 [cockpit-deploy-github-history.mdc](file:///c:/Users/aliceemoce/dev/cockpit-tools/.cursor/rules/cockpit-deploy-github-history.mdc)（`npm run build` + `npm run tauri build` 覆盖安装）
   - 三个外部续费程序：工作区副本（`external/xubei_and_renewal/`）要能自动从源头更新/覆盖
   - Nirvana Proxy：与无忧同源体系时，更新不得漏掉 `nirvana-proxy.exe`（本机 bin 与工作区 `decompiled/nirvana-proxy/` 副本）

## 对照文档

- [无感换号对照_XuBei_vs_续杯助手.md](file:///c:/Users/aliceemoce/dev/cockpit-tools/external/xubei_and_renewal/02_续杯助手_无感/无感换号对照_XuBei_vs_续杯助手.md)
- [external/xubei_and_renewal/README.md](file:///c:/Users/aliceemoce/dev/cockpit-tools/external/xubei_and_renewal/README.md)

## 禁止

- 禁止不参考这五个程序就自己从头写切号/换号/代理逻辑（已有成熟实现不参考 = 重复造轮子）
- 禁止删除无忧小助手已对齐的切号逻辑
- 禁止把 Nirvana Proxy 与无忧小助手混成同一个程序
- 禁止幻想某个程序的做法——必须实际读它的代码/解包/文档后再参考
