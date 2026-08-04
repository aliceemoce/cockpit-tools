# NVIDIA API 全量 121 模型对照表

说明：NVIDIA `/v1/models` **不返回参数量**，本表合并：
1. 官方/论文已公布规格（标注来源）
2. 模型名中的 `397b-a17b` = 总参 397B / 激活 17B
3. 模型名中的 `70b` 等 = 稠密参数量估计
4. 无法解析的标「未公开」，**不编造数字**

对话性能档位按 **激活参数量**（MoE）或总参（稠密）粗分 1～5，仅用于同类对比。

## 全表

| 模型 ID | 类型 | 总参(B) | 激活(B) | 对话档位 | 规格来源 |
|---|---|---:|---:|---|---|
| `mistralai/mistral-large-3-675b-instruct-2512` | 通用对话 | 675 | 675 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nemotron-4-340b-instruct` | 通用对话 | 340 | 340 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-3.1-nemotron-ultra-253b-v1` | 通用对话 | 253 | 253 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `mistralai/mistral-medium-3.5-128b` | 通用对话 | 128 | 128 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `writer/palmyra-creative-122b` | 通用对话 | 122 | 122 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `openai/gpt-oss-120b` | 通用对话 | 120 | 120 | 5-旗舰 | 名称 120B |
| `mistralai/mistral-small-4-119b-2603` | 通用对话 | 119 | 119 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `stockmark/stockmark-2-100b-instruct` | 通用对话 | 100 | 100 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `abacusai/dracarys-llama-3.1-70b-instruct` | 通用对话 | 70 | 70 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `meta/codellama-70b` | 代码向 | 70 | 70 | 5-旗舰 | 名称 70B 稠密 |
| `meta/llama-3.1-70b-instruct` | 通用对话 | 70 | 70 | 5-旗舰 | 名称 70B 稠密 |
| `meta/llama-3.3-70b-instruct` | 通用对话 | 70 | 70 | 5-旗舰 | 名称 70B 稠密 |
| `meta/llama2-70b` | 通用对话 | 70 | 70 | 5-旗舰 | 名称 70B 稠密 |
| `nvidia/llama-3.1-nemotron-70b-instruct` | 通用对话 | 70 | 70 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama3-chatqa-1.5-70b` | 通用对话 | 70 | 70 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `writer/palmyra-fin-70b-32k` | 通用对话 | 70 | 70 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `writer/palmyra-med-70b` | 通用对话 | 70 | 70 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `writer/palmyra-med-70b-32k` | 通用对话 | 70 | 70 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nemotron-3-ultra-550b-a55b` | 通用对话 | 550 | 55 | 5-旗舰 | 从模型名 MoE 模式解析 |
| `nvidia/llama-3.1-nemotron-51b-instruct` | 通用对话 | 51 | 51 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `deepseek-ai/deepseek-v4-pro` | 通用对话 | 1600 | 49 | 5-旗舰 | DeepSeek 官方: 1.6T / 49B active |
| `nvidia/llama-3.3-nemotron-super-49b-v1` | 通用对话 | 49 | 49 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-3.3-nemotron-super-49b-v1.5` | 通用对话 | 49 | 49 | 5-旗舰 | 从模型名 *B 解析（稠密估计） |
| `databricks/dbrx-instruct` | 通用对话 | 132 | 36 | 4-很强 | DBRX 公开: 132B total / 36B active MoE |
| `bytedance/seed-oss-36b-instruct` | 通用对话 | 36 | 36 | 4-很强 | 从模型名 *B 解析（稠密估计） |
| `ibm/granite-34b-code-instruct` | 代码向 | 34 | 34 | 4-很强 | 从模型名 *B 解析（稠密估计） |
| `google/gemma-4-31b-it` | 通用对话 | 31 | 31 | 4-很强 | 从模型名 *B 解析（稠密估计） |
| `mistralai/codestral-22b-instruct-v0.1` | 代码向 | 22 | 22 | 4-很强 | 名称 22B 稠密 |
| `mistralai/mixtral-8x22b-v0.1` | 通用对话 | 22 | 22 | 4-很强 | 从模型名 *B 解析（稠密估计） |
| `openai/gpt-oss-20b` | 通用对话 | 20 | 20 | 4-很强 | 名称 20B |
| `qwen/qwen3.5-397b-a17b` | 通用对话 | 397 | 17 | 4-很强 | 从模型名 MoE 模式解析 |
| `meta/llama-4-maverick-17b-128e-instruct` | 通用对话 | 17 | 17 | 4-很强 | 从模型名 *B 解析（稠密估计） |
| `bigcode/starcoder2-15b` | 代码向 | 15 | 15 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `mistralai/ministral-14b-instruct-2512` | 通用对话 | 14 | 14 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `deepseek-ai/deepseek-v4-flash` | 通用对话 | 284 | 13 | 3-中等 | DeepSeek 官方: 284B / 13B active |
| `nvidia/nemotron-3-super-120b-a12b` | 通用对话 | 120 | 12 | 3-中等 | 从模型名 MoE 模式解析 |
| `google/gemma-3-12b-it` | 通用对话 | 12 | 12 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `nv-mistralai/mistral-nemo-12b-instruct` | 通用对话 | 12 | 12 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `upstage/solar-10.7b-instruct` | 通用对话 | 10.7 | 10.7 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `qwen/qwen3.5-122b-a10b` | 通用对话 | 122 | 10 | 3-中等 | 从模型名 MoE 模式解析 |
| `nvidia/nvidia-nemotron-nano-9b-v2` | 通用对话 | 9 | 9 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `ibm/granite-3.0-8b-instruct` | 通用对话 | 8 | 8 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `ibm/granite-8b-code-instruct` | 代码向 | 8 | 8 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `meta/llama-3.1-8b-instruct` | 通用对话 | 8 | 8 | 3-中等 | 名称 8B 稠密 |
| `nvidia/cosmos-reason2-8b` | 通用对话 | 8 | 8 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-3.1-nemotron-nano-8b-v1` | 通用对话 | 8 | 8 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `nvidia/mistral-nemo-minitron-8b-8k-instruct` | 通用对话 | 8 | 8 | 3-中等 | 从模型名 *B 解析（稠密估计） |
| `aisingapore/sea-lion-7b-instruct` | 通用对话 | 7 | 7 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `google/codegemma-1.1-7b` | 代码向 | 7 | 7 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `google/codegemma-7b` | 代码向 | 7 | 7 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `mistralai/mistral-7b-instruct-v0.3` | 通用对话 | 7 | 7 | 2-轻量 | 名称 7B 稠密 |
| `mistralai/mixtral-8x7b-instruct-v0.1` | 通用对话 | 7 | 7 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `zyphra/zamba2-7b-instruct` | 通用对话 | 7 | 7 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `deepseek-ai/deepseek-coder-6.7b-instruct` | 代码向 | 6.7 | 6.7 | 2-轻量 | 名称 6.7B 稠密 |
| `google/diffusiongemma-26b-a4b-it` | 通用对话 | 26 | 4 | 2-轻量 | 从模型名 MoE 模式解析 |
| `google/gemma-3-4b-it` | 通用对话 | 4 | 4 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `google/gemma-3n-e4b-it` | 通用对话 | 4 | 4 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nemotron-mini-4b-instruct` | 通用对话 | 4 | 4 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `qwen/qwen3-next-80b-a3b-instruct` | 通用对话 | 80 | 3 | 2-轻量 | 从模型名 MoE 模式解析 |
| `nvidia/nemotron-3-nano-30b-a3b` | 通用对话 | 30 | 3 | 2-轻量 | 从模型名 MoE 模式解析 |
| `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning` | 通用对话 | 30 | 3 | 2-轻量 | 从模型名 MoE 模式解析 |
| `nvidia/nemotron-nano-3-30b-a3b` | 通用对话 | 30 | 3 | 2-轻量 | 从模型名 MoE 模式解析 |
| `ibm/granite-3.0-3b-a800m-instruct` | 通用对话 | 3 | 3 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `meta/llama-3.2-3b-instruct` | 通用对话 | 3 | 3 | 2-轻量 | 从模型名 *B 解析（稠密估计） |
| `google/gemma-2-2b-it` | 通用对话 | 2 | 2 | 1-迷你 | 从模型名 *B 解析（稠密估计） |
| `google/gemma-2b` | 通用对话 | 2 | 2 | 1-迷你 | 从模型名 *B 解析（稠密估计） |
| `google/gemma-3n-e2b-it` | 通用对话 | 2 | 2 | 1-迷你 | 从模型名 *B 解析（稠密估计） |
| `google/recurrentgemma-2b` | 通用对话 | 2 | 2 | 1-迷你 | 从模型名 *B 解析（稠密估计） |
| `meta/llama-3.2-1b-instruct` | 通用对话 | 1 | 1 | 1-迷你 | 从模型名 *B 解析（稠密估计） |
| `01-ai/yi-large` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `adept/fuyu-8b` | 视觉 | 8 | 8 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `ai21labs/jamba-1.5-large-instruct` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `baai/bge-m3` | 向量嵌入 |  |  | -非对话 | 名称无参数标注 |
| `google/deplot` | 视觉 |  |  | -非对话 | 名称无参数标注 |
| `meta/llama-3.2-11b-vision-instruct` | 视觉 | 11 | 11 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `meta/llama-3.2-90b-vision-instruct` | 视觉 | 90 | 90 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `meta/llama-guard-4-12b` | 安全审核 | 12 | 12 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `microsoft/kosmos-2` | 视觉 |  |  | -非对话 | 名称无参数标注 |
| `microsoft/phi-3-vision-128k-instruct` | 视觉 |  |  | -非对话 | 名称无参数标注 |
| `microsoft/phi-3.5-moe-instruct` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `microsoft/phi-4-mini-instruct` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `microsoft/phi-4-multimodal-instruct` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `minimaxai/minimax-m2.7` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `minimaxai/minimax-m3` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `mistralai/mistral-large` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `mistralai/mistral-large-2-instruct` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `mistralai/mistral-nemotron` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `moonshotai/kimi-k2.6` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `nvidia/ai-synthetic-video-detector` | 安全审核 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/embed-qa-4` | 向量嵌入 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/gliner-pii` | 安全审核 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/ising-calibration-1-35b-a3b` | 工具/其他 | 35 | 3 | -非对话 | 从模型名 MoE 模式解析 |
| `nvidia/llama-3.1-nemoguard-8b-content-safety` | 安全审核 | 8 | 8 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-3.1-nemoguard-8b-topic-control` | 安全审核 | 8 | 8 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-3.1-nemotron-nano-vl-8b-v1` | 视觉 | 8 | 8 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-3.1-nemotron-safety-guard-8b-v3` | 安全审核 | 8 | 8 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-3.2-nemoretriever-1b-vlm-embed-v1` | 向量嵌入 | 1 | 1 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-3.2-nv-embedqa-1b-v1` | 向量嵌入 | 1 | 1 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-nemotron-embed-1b-v2` | 向量嵌入 | 1 | 1 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/llama-nemotron-embed-vl-1b-v2` | 向量嵌入 | 1 | 1 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nemoretriever-parse` | 向量嵌入 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/nemotron-3-content-safety` | 安全审核 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/nemotron-3.5-content-safety` | 安全审核 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/nemotron-4-340b-reward` | 工具/其他 | 340 | 340 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nemotron-content-safety-reasoning-4b` | 安全审核 | 4 | 4 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nemotron-nano-12b-v2-vl` | 视觉 | 12 | 12 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nemotron-parse` | 工具/其他 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/neva-22b` | 视觉 | 22 | 22 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nv-embed-v1` | 向量嵌入 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/nv-embedcode-7b-v1` | 向量嵌入 | 7 | 7 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nv-embedqa-e5-v5` | 向量嵌入 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/nv-embedqa-mistral-7b-v2` | 向量嵌入 | 7 | 7 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/nvclip` | 工具/其他 |  |  | -非对话 | 名称无参数标注 |
| `nvidia/riva-translate-4b-instruct` | 翻译 | 4 | 4 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/riva-translate-4b-instruct-v1.1` | 翻译 | 4 | 4 | -非对话 | 从模型名 *B 解析（稠密估计） |
| `nvidia/vila` | 视觉 |  |  | -非对话 | 名称无参数标注 |
| `sarvamai/sarvam-m` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `snowflake/arctic-embed-l` | 向量嵌入 |  |  | -非对话 | 名称无参数标注 |
| `stepfun-ai/step-3.5-flash` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `stepfun-ai/step-3.7-flash` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |
| `z-ai/glm-5.1` | 通用对话 |  |  | ?未公开 | 名称无参数标注 |

## 对话/代码向 — 按档位排序（完整）

### 档位 5（旗舰）— 23 个

- `mistralai/mistral-large-3-675b-instruct-2512` — 总 675B（稠密） — 从模型名 *B 解析（稠密估计）
- `nvidia/nemotron-4-340b-instruct` — 总 340B（稠密） — 从模型名 *B 解析（稠密估计）
- `nvidia/llama-3.1-nemotron-ultra-253b-v1` — 总 253B（稠密） — 从模型名 *B 解析（稠密估计）
- `mistralai/mistral-medium-3.5-128b` — 总 128B（稠密） — 从模型名 *B 解析（稠密估计）
- `writer/palmyra-creative-122b` — 总 122B（稠密） — 从模型名 *B 解析（稠密估计）
- `openai/gpt-oss-120b` — 总 120B（稠密） — 名称 120B
- `mistralai/mistral-small-4-119b-2603` — 总 119B（稠密） — 从模型名 *B 解析（稠密估计）
- `stockmark/stockmark-2-100b-instruct` — 总 100B（稠密） — 从模型名 *B 解析（稠密估计）
- `abacusai/dracarys-llama-3.1-70b-instruct` — 总 70B（稠密） — 从模型名 *B 解析（稠密估计）
- `meta/codellama-70b` — 总 70B（稠密） — 名称 70B 稠密
- `meta/llama-3.1-70b-instruct` — 总 70B（稠密） — 名称 70B 稠密
- `meta/llama-3.3-70b-instruct` — 总 70B（稠密） — 名称 70B 稠密
- `meta/llama2-70b` — 总 70B（稠密） — 名称 70B 稠密
- `nvidia/llama-3.1-nemotron-70b-instruct` — 总 70B（稠密） — 从模型名 *B 解析（稠密估计）
- `nvidia/llama3-chatqa-1.5-70b` — 总 70B（稠密） — 从模型名 *B 解析（稠密估计）
- `writer/palmyra-fin-70b-32k` — 总 70B（稠密） — 从模型名 *B 解析（稠密估计）
- `writer/palmyra-med-70b` — 总 70B（稠密） — 从模型名 *B 解析（稠密估计）
- `writer/palmyra-med-70b-32k` — 总 70B（稠密） — 从模型名 *B 解析（稠密估计）
- `nvidia/nemotron-3-ultra-550b-a55b` — 总 550B，激活 55B — 从模型名 MoE 模式解析
- `nvidia/llama-3.1-nemotron-51b-instruct` — 总 51B（稠密） — 从模型名 *B 解析（稠密估计）
- `deepseek-ai/deepseek-v4-pro` — 总 1600B，激活 49B — DeepSeek 官方: 1.6T / 49B active
- `nvidia/llama-3.3-nemotron-super-49b-v1` — 总 49B（稠密） — 从模型名 *B 解析（稠密估计）
- `nvidia/llama-3.3-nemotron-super-49b-v1.5` — 总 49B（稠密） — 从模型名 *B 解析（稠密估计）

### 档位 4（很强）— 9 个

- `databricks/dbrx-instruct` — 总 132B，激活 36B — DBRX 公开: 132B total / 36B active MoE
- `bytedance/seed-oss-36b-instruct` — 总 36B（稠密） — 从模型名 *B 解析（稠密估计）
- `ibm/granite-34b-code-instruct` — 总 34B（稠密） — 从模型名 *B 解析（稠密估计）
- `google/gemma-4-31b-it` — 总 31B（稠密） — 从模型名 *B 解析（稠密估计）
- `mistralai/codestral-22b-instruct-v0.1` — 总 22B（稠密） — 名称 22B 稠密
- `mistralai/mixtral-8x22b-v0.1` — 总 22B（稠密） — 从模型名 *B 解析（稠密估计）
- `openai/gpt-oss-20b` — 总 20B（稠密） — 名称 20B
- `qwen/qwen3.5-397b-a17b` — 总 397B，激活 17B — 从模型名 MoE 模式解析
- `meta/llama-4-maverick-17b-128e-instruct` — 总 17B（稠密） — 从模型名 *B 解析（稠密估计）

### 档位 3（中等）— 15 个

- `bigcode/starcoder2-15b` — 总 15B（稠密） — 从模型名 *B 解析（稠密估计）
- `mistralai/ministral-14b-instruct-2512` — 总 14B（稠密） — 从模型名 *B 解析（稠密估计）
- `deepseek-ai/deepseek-v4-flash` — 总 284B，激活 13B — DeepSeek 官方: 284B / 13B active
- `nvidia/nemotron-3-super-120b-a12b` — 总 120B，激活 12B — 从模型名 MoE 模式解析
- `google/gemma-3-12b-it` — 总 12B（稠密） — 从模型名 *B 解析（稠密估计）
- `nv-mistralai/mistral-nemo-12b-instruct` — 总 12B（稠密） — 从模型名 *B 解析（稠密估计）
- `upstage/solar-10.7b-instruct` — 总 10.7B（稠密） — 从模型名 *B 解析（稠密估计）
- `qwen/qwen3.5-122b-a10b` — 总 122B，激活 10B — 从模型名 MoE 模式解析
- `nvidia/nvidia-nemotron-nano-9b-v2` — 总 9B（稠密） — 从模型名 *B 解析（稠密估计）
- `ibm/granite-3.0-8b-instruct` — 总 8B（稠密） — 从模型名 *B 解析（稠密估计）
- `ibm/granite-8b-code-instruct` — 总 8B（稠密） — 从模型名 *B 解析（稠密估计）
- `meta/llama-3.1-8b-instruct` — 总 8B（稠密） — 名称 8B 稠密
- `nvidia/cosmos-reason2-8b` — 总 8B（稠密） — 从模型名 *B 解析（稠密估计）
- `nvidia/llama-3.1-nemotron-nano-8b-v1` — 总 8B（稠密） — 从模型名 *B 解析（稠密估计）
- `nvidia/mistral-nemo-minitron-8b-8k-instruct` — 总 8B（稠密） — 从模型名 *B 解析（稠密估计）

### 档位 2（轻量）— 17 个

- `aisingapore/sea-lion-7b-instruct` — 总 7B（稠密） — 从模型名 *B 解析（稠密估计）
- `google/codegemma-1.1-7b` — 总 7B（稠密） — 从模型名 *B 解析（稠密估计）
- `google/codegemma-7b` — 总 7B（稠密） — 从模型名 *B 解析（稠密估计）
- `mistralai/mistral-7b-instruct-v0.3` — 总 7B（稠密） — 名称 7B 稠密
- `mistralai/mixtral-8x7b-instruct-v0.1` — 总 7B（稠密） — 从模型名 *B 解析（稠密估计）
- `zyphra/zamba2-7b-instruct` — 总 7B（稠密） — 从模型名 *B 解析（稠密估计）
- `deepseek-ai/deepseek-coder-6.7b-instruct` — 总 6.7B（稠密） — 名称 6.7B 稠密
- `google/diffusiongemma-26b-a4b-it` — 总 26B，激活 4B — 从模型名 MoE 模式解析
- `google/gemma-3-4b-it` — 总 4B（稠密） — 从模型名 *B 解析（稠密估计）
- `google/gemma-3n-e4b-it` — 总 4B（稠密） — 从模型名 *B 解析（稠密估计）
- `nvidia/nemotron-mini-4b-instruct` — 总 4B（稠密） — 从模型名 *B 解析（稠密估计）
- `qwen/qwen3-next-80b-a3b-instruct` — 总 80B，激活 3B — 从模型名 MoE 模式解析
- `nvidia/nemotron-3-nano-30b-a3b` — 总 30B，激活 3B — 从模型名 MoE 模式解析
- `nvidia/nemotron-3-nano-omni-30b-a3b-reasoning` — 总 30B，激活 3B — 从模型名 MoE 模式解析
- `nvidia/nemotron-nano-3-30b-a3b` — 总 30B，激活 3B — 从模型名 MoE 模式解析
- `ibm/granite-3.0-3b-a800m-instruct` — 总 3B（稠密） — 从模型名 *B 解析（稠密估计）
- `meta/llama-3.2-3b-instruct` — 总 3B（稠密） — 从模型名 *B 解析（稠密估计）

### 档位 1（迷你）— 5 个

- `google/gemma-2-2b-it` — 总 2B（稠密） — 从模型名 *B 解析（稠密估计）
- `google/gemma-2b` — 总 2B（稠密） — 从模型名 *B 解析（稠密估计）
- `google/gemma-3n-e2b-it` — 总 2B（稠密） — 从模型名 *B 解析（稠密估计）
- `google/recurrentgemma-2b` — 总 2B（稠密） — 从模型名 *B 解析（稠密估计）
- `meta/llama-3.2-1b-instruct` — 总 1B（稠密） — 从模型名 *B 解析（稠密估计）

### 对话向但参数量未公开 — 15 个

- `01-ai/yi-large`
- `ai21labs/jamba-1.5-large-instruct`
- `microsoft/phi-3.5-moe-instruct`
- `microsoft/phi-4-mini-instruct`
- `microsoft/phi-4-multimodal-instruct`
- `minimaxai/minimax-m2.7`
- `minimaxai/minimax-m3`
- `mistralai/mistral-large`
- `mistralai/mistral-large-2-instruct`
- `mistralai/mistral-nemotron`
- `moonshotai/kimi-k2.6`
- `sarvamai/sarvam-m`
- `stepfun-ai/step-3.5-flash`
- `stepfun-ai/step-3.7-flash`
- `z-ai/glm-5.1`

## 非对话主模型 — 37 个（不适合当 Codex 主脑）

- `adept/fuyu-8b`（视觉）
- `baai/bge-m3`（向量嵌入）
- `google/deplot`（视觉）
- `meta/llama-3.2-11b-vision-instruct`（视觉）
- `meta/llama-3.2-90b-vision-instruct`（视觉）
- `meta/llama-guard-4-12b`（安全审核）
- `microsoft/kosmos-2`（视觉）
- `microsoft/phi-3-vision-128k-instruct`（视觉）
- `nvidia/ai-synthetic-video-detector`（安全审核）
- `nvidia/embed-qa-4`（向量嵌入）
- `nvidia/gliner-pii`（安全审核）
- `nvidia/ising-calibration-1-35b-a3b`（工具/其他）
- `nvidia/llama-3.1-nemoguard-8b-content-safety`（安全审核）
- `nvidia/llama-3.1-nemoguard-8b-topic-control`（安全审核）
- `nvidia/llama-3.1-nemotron-nano-vl-8b-v1`（视觉）
- `nvidia/llama-3.1-nemotron-safety-guard-8b-v3`（安全审核）
- `nvidia/llama-3.2-nemoretriever-1b-vlm-embed-v1`（向量嵌入）
- `nvidia/llama-3.2-nv-embedqa-1b-v1`（向量嵌入）
- `nvidia/llama-nemotron-embed-1b-v2`（向量嵌入）
- `nvidia/llama-nemotron-embed-vl-1b-v2`（向量嵌入）
- `nvidia/nemoretriever-parse`（向量嵌入）
- `nvidia/nemotron-3-content-safety`（安全审核）
- `nvidia/nemotron-3.5-content-safety`（安全审核）
- `nvidia/nemotron-4-340b-reward`（工具/其他）
- `nvidia/nemotron-content-safety-reasoning-4b`（安全审核）
- `nvidia/nemotron-nano-12b-v2-vl`（视觉）
- `nvidia/nemotron-parse`（工具/其他）
- `nvidia/neva-22b`（视觉）
- `nvidia/nv-embed-v1`（向量嵌入）
- `nvidia/nv-embedcode-7b-v1`（向量嵌入）
- `nvidia/nv-embedqa-e5-v5`（向量嵌入）
- `nvidia/nv-embedqa-mistral-7b-v2`（向量嵌入）
- `nvidia/nvclip`（工具/其他）
- `nvidia/riva-translate-4b-instruct`（翻译）
- `nvidia/riva-translate-4b-instruct-v1.1`（翻译）
- `nvidia/vila`（视觉）
- `snowflake/arctic-embed-l`（向量嵌入）
