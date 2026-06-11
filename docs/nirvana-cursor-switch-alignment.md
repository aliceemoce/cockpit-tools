# 无忧小助手 Cursor 切号 vs Cockpit（源码对齐）

> cockpit-tools **不包含**任何外部小助手模块。本节从本机 `nirvana.exe` v2.7.8 解压的 `main-FXxcqbQA.js` 提取，并已写入 Rust 实现。

## 无忧「传统切号」真实流程（`cursor:accounts:switch` → 函数 `i()` @20973）

1. `closeCursor`（`ho`）— `taskkill /IM Cursor.exe` + 等待退出
2. `switchTokensInDb`（`Kh`）— 删 `-wal/-shm`；删旧 `cursorAuth/*`（**不删** `onboardingDate`）；重置 `storage.serviceMachineId` + `telemetry.*`；写入新 token
3. `resetStorageJsonIds`（`Gh`）— **独立**生成 telemetry 写入 `storage.json`
4. `resetMachineIdFile`（`Jh`）— **独立**生成 UUID 写入 `%AppData%\Cursor\machineId`
5. `patchCursorMachineId`（`Yh`）— patch `resources/app/out/main.js`（**不是** workbench）
6. `resetWindowsMachineGuid`（`Nc`）— PowerShell 读写注册表 + 备份到 `%USERPROFILE%\MachineGuid_Backups`（需管理员，失败可跳过）
7. 等待 **1500ms**
8. `startCursor`（`go`）

### 不在传统切号里的步骤

| 功能 | 用途 |
|------|------|
| `cleanCursorEnvironment`（`Vh`） | 环境清理页 / `cursor:cleaner:run`，会删更多 vscdb 键 |
| `patchCursorWorkbench`（`Xh`） | 无感换号 / seamless 安装，传统切号**不调用** |
| `silentSwitch`（`Zh`） | 密码静默登录：关进程 → `writeTokenToDb` → 启动（不做指纹重置） |

## Cockpit 对应实现

| 无忧步骤 | Cockpit 模块 |
|----------|--------------|
| 1 | `cursor_instance::close_cursor` |
| 2 | `cursor_account::switch_tokens_in_profile_db` |
| 2（多开） | `ensure_state_db_for_injection`（profile 无 vscdb 时先复制，Cockpit 扩展） |
| 3 | `cursor_account::reset_storage_json_ids_for_profile` |
| 4 | `cursor_account::reset_machine_id_file_for_profile` |
| 5–6 | `cursor_switch_align::apply_nirvana_traditional_switch_patches` |
| 7 | `cursor_start_instance_prepared` 内 `sleep(1500ms)` |
| 8 | `cursor_start_instance_prepared` → `start_cursor_*` |

总览 Play 与多开 Start 统一入口：`start_cursor_instance_with_account_switch` → `switch_cursor_account_to_profile`。

## 后端验证（无 UI）

```powershell
python scripts\verify_cursor_switch_paths.py
python scripts\verify_backend_cursor_switch.py
```

报告：`scripts/backend_verify_report.json`（`flow_matches_nirvana: true` 为通过）
