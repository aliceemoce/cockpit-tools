# Cockpit Tools — 会话简报

**日期**: 2026-06-15  
**验收 exe**: `Desktop\Cockpit-nirvana-token-test.exe` SHA `F5256511…`  
**源码**: `agent-build-20260615` @ `44321ebf`  
**PR**: https://github.com/aliceemoce/cockpit-tools/pull/2（head 改 `agent-build-20260615`）  
**对照**: https://github.com/aliceemoce/cockpit-tools/compare/nirvana-baseline-20260615...agent-build-20260615

## 合并包能力

- 默认 `close_cursor` 只关本 profile；多开 strict close
- Kh 裸 JWT + 落盘校验；满额换号；无 scheduler
- transient 配额失败不写盘；emit 仅 persisted
- 无全局 `accounts:changed` listener

## 历史备份

- `Desktop\Cockpit-nirvana-token-20260614\Cockpit-nirvana-token-test.exe` — `9791D94F`（对照用，勿删）

## 拒收/误标

- `CB525188` staging r2、`7761f453`、`94B7D986` 中间 Agent 包
