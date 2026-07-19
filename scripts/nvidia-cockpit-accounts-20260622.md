# Cockpit NVIDIA API 双账号配置说明

更新日期：2026-06-22

## 账号一览

| 显示名 | 账号 ID | 来源 | 状态 |
|---|---|---|---|
| api.openai.com（NVIDIA API） | codex_apikey_744ece4e421cb34fbe11f5a88bc06884 | 原有 | 保留，未替换 |
| NVIDIA API (自建) | codex_apikey_fd0694cfc693acf251898c5cf171f242 | 本次新增 | 已写入 Cockpit |

## 接口

- Base URL：`https://integrate.api.nvidia.com/v1`
- Wire API：`chat_completions`（经 Provider Gateway）
- 模型目录：121 个（与 NVIDIA `/v1/models` 同步）
- 默认上游模型：`deepseek-ai/deepseek-v4-flash`

## Key 过期（只能你在 NVIDIA 官网改）

Cockpit **不能**把已有 Key 改成永久不过期。过期策略在 [build.nvidia.com](https://build.nvidia.com) → API Keys 生成时选择：

- 推荐选 **Never Expire / 永不过期**
- 若旧 Key 已设 6 个月，只能重新生成新 Key 再更新到 Cockpit

## 额度（人话）

- 免费试用，不按 token 扣钱
- 限速约 40 次/分钟/模型（实际会波动）
- 无「剩余额度百分比」可查；Cockpit 显示「等待查询额度」属正常

## 本地路径

- 账号索引：`%USERPROFILE%\.antigravity_cockpit\codex_accounts.json`
- 账号文件：`%USERPROFILE%\.antigravity_cockpit\codex_accounts\codex_apikey_*.json`
- 供应商：`%USERPROFILE%\.antigravity_cockpit\codex_model_providers.json`

## 维护脚本

- `scripts/_patch_nvidia_models.py` — 刷新 121 个模型到所有 NVIDIA 账号
- `scripts/_add_nvidia_self_account.py` — 添加自建 NVIDIA 账号（不覆盖旧账号）

## 密钥存放

完整 API Key 仅保存在本机 Cockpit 数据目录与私有凭证仓 `cockpit-credentials`，**不进入公开 GitHub 仓库**。
