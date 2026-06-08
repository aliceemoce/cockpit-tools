# 外部续杯工具 Cursor 切号 vs Cockpit（实测对齐）

> cockpit-tools **不包含**任何外部小助手模块。本节记录从本机已安装 `nirvana.exe` v2.7.8 (`JZzCG/windsurf-assistant`) **解压 app.asar 后**读到的逻辑，并已写入 `cursor_switch_align.rs`。

## 无忧小助手真实流程（从 `main-FXxcqbQA.js` 提取）

1. `taskkill /IM Cursor.exe`（`closeCursor` / `ho`）
2. `resetMachineIdFile` — 写 `%AppData%\Cursor\machineId`
3. `resetStorageJsonIds` — `storage.json` 的 `telemetry.machineId/sqmId/devDeviceId`
4. `state.vscdb` — `storage.serviceMachineId` + 全部 `telemetry.*`
5. `resetWindowsMachineGuid` — 注册表 `HKLM\...\Cryptography\MachineGuid`（需管理员，失败可跳过）
6. `patchCursorWorkbench` — 在 `workbench.desktop.main.js` 的 `this.logout=` 前注入 `__cursorAuthBridge.switchAccount`
7. `writeTokenToDb` / `switchTokensInDb` — 写 `cursorAuth/accessToken|refreshToken|cachedEmail|cachedSignUpType`
8. `startCursor`（`go`）

## Cockpit 对应实现

| 步骤 | 模块 |
|------|------|
| 1 | `cursor_instance::close_cursor` |
| 2–4 | `hard_reset_cursor_fingerprint_state_for_profile` |
| 5–6 | `cursor_switch_align::apply_pre_inject_cursor_patches` |
| 7 | `inject_account_to_profile` → `write_cursor_auth_fields_to_conn` |
| 8 | `cursor_start_instance_prepared` |

总览 Play 与多开 Start 统一入口：`start_cursor_instance_with_account_switch`。

## 后端验证（无 UI、无鼠标）

```powershell
python scripts\verify_cursor_switch_paths.py
python scripts\verify_backend_cursor_switch.py
```

报告：`scripts/backend_verify_report.json`（`flow_matches_nirvana: true` 为通过）
