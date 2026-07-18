#!/usr/bin/env python3
"""Build full 121-model comparison matrix from API list + published specs + name parsing."""
import json
import pathlib
import re
from dataclasses import dataclass, asdict

ROOT = pathlib.Path(__file__).resolve().parents[1]
MODELS_JSON = ROOT / "scripts/_nvidia_models_full.json"
OUT_JSON = ROOT / "scripts/nvidia-models-full-matrix.json"
OUT_MD = ROOT / "scripts/nvidia-models-full-matrix.md"

# Published specs: (total_B, active_B or None, source note)
PUBLISHED: dict[str, tuple[float | None, float | None, str]] = {
    "deepseek-ai/deepseek-v4-pro": (1600, 49, "DeepSeek 官方: 1.6T / 49B active"),
    "deepseek-ai/deepseek-v4-flash": (284, 13, "DeepSeek 官方: 284B / 13B active"),
    "deepseek-ai/deepseek-coder-6.7b-instruct": (6.7, 6.7, "名称 6.7B 稠密"),
    "meta/llama-3.3-70b-instruct": (70, 70, "名称 70B 稠密"),
    "meta/llama-3.1-70b-instruct": (70, 70, "名称 70B 稠密"),
    "meta/llama-3.1-8b-instruct": (8, 8, "名称 8B 稠密"),
    "meta/codellama-70b": (70, 70, "名称 70B 稠密"),
    "meta/llama2-70b": (70, 70, "名称 70B 稠密"),
    "mistralai/mistral-7b-instruct-v0.3": (7, 7, "名称 7B 稠密"),
    "mistralai/codestral-22b-instruct-v0.1": (22, 22, "名称 22B 稠密"),
    "openai/gpt-oss-120b": (120, 120, "名称 120B"),
    "openai/gpt-oss-20b": (20, 20, "名称 20B"),
    "databricks/dbrx-instruct": (132, 36, "DBRX 公开: 132B total / 36B active MoE"),
}

MOE_RE = re.compile(r"(?P<total>\d+(?:\.\d+)?)b-a(?P<active>\d+(?:\.\d+)?)b", re.I)
DENSE_RE = re.compile(r"(?P<size>\d+(?:\.\d+)?)b", re.I)


def category(model_id: str) -> str:
    low = model_id.lower()
    if any(x in low for x in ("embed", "bge-", "arctic-embed", "nv-embed", "nemoretriever")):
        return "embedding"
    if any(x in low for x in ("guard", "content-safety", "safety", "gliner", "detector")):
        return "safety"
    if any(x in low for x in ("translate", "riva-translate")):
        return "translation"
    if any(x in low for x in ("vision", "vl-", "-vl", "vila", "kosmos", "neva", "fuyu", "deplot")):
        return "vision"
    if any(x in low for x in ("parse", "nvclip", "reward", "calibration")):
        return "tooling"
    if "coder" in low or "codestral" in low or "code" in low or "granite" in low and "code" in low:
        return "code_chat"
    return "general_chat"


def parse_name(model_id: str) -> tuple[float | None, float | None, str]:
    m = MOE_RE.search(model_id)
    if m:
        total = float(m.group("total"))
        active = float(m.group("active"))
        return total, active, "从模型名 MoE 模式解析"
    sizes = [float(x.group("size")) for x in DENSE_RE.finditer(model_id)]
    if sizes:
        # use largest number in id as dense size heuristic
        size = max(sizes)
        return size, size, "从模型名 *B 解析（稠密估计）"
    return None, None, "名称无参数标注"


def tier_for_chat(total: float | None, active: float | None, cat: str) -> tuple[int, str]:
    if cat not in ("general_chat", "code_chat"):
        return 0, "非对话主模型"
    score = active or total
    if score is None:
        return -1, "参数量未公开"
    if score >= 40:
        return 5, "旗舰"
    if score >= 17:
        return 4, "很强"
    if score >= 8:
        return 3, "中等"
    if score >= 3:
        return 2, "轻量"
    return 1, "迷你"


def main() -> None:
    models = json.loads(MODELS_JSON.read_text(encoding="utf-8"))
    rows = []
    for model_id in models:
        vendor = model_id.split("/")[0] if "/" in model_id else model_id
        cat = category(model_id)
        pub = PUBLISHED.get(model_id)
        if pub:
            total_b, active_b, spec_src = pub
        else:
            total_b, active_b, spec_src = parse_name(model_id)
        tier, tier_label = tier_for_chat(total_b, active_b, cat)
        rows.append(
            {
                "model_id": model_id,
                "vendor": vendor,
                "category": cat,
                "total_params_B": total_b,
                "active_params_B": active_b,
                "spec_source": spec_src,
                "chat_tier": tier,
                "chat_tier_label": tier_label,
            }
        )

    # sort chat models by active then total desc
    def sort_key(r: dict) -> tuple:
        if r["chat_tier"] <= 0:
            return (1, 0, 0, r["model_id"])
        active = r["active_params_B"] or 0
        total = r["total_params_B"] or 0
        return (0, -r["chat_tier"], -active, -total, r["model_id"])

    rows.sort(key=sort_key)

    OUT_JSON.write_text(json.dumps(rows, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    lines = [
        "# NVIDIA API 全量 121 模型对照表",
        "",
        "说明：NVIDIA `/v1/models` **不返回参数量**，本表合并：",
        "1. 官方/论文已公布规格（标注来源）",
        "2. 模型名中的 `397b-a17b` = 总参 397B / 激活 17B",
        "3. 模型名中的 `70b` 等 = 稠密参数量估计",
        "4. 无法解析的标「未公开」，**不编造数字**",
        "",
        "对话性能档位按 **激活参数量**（MoE）或总参（稠密）粗分 1～5，仅用于同类对比。",
        "",
        "## 全表",
        "",
        "| 模型 ID | 类型 | 总参(B) | 激活(B) | 对话档位 | 规格来源 |",
        "|---|---|---:|---:|---|---|",
    ]
    tier_map = {5: "5-旗舰", 4: "4-很强", 3: "3-中等", 2: "2-轻量", 1: "1-迷你", 0: "-非对话", -1: "?未公开"}
    cat_map = {
        "general_chat": "通用对话",
        "code_chat": "代码向",
        "embedding": "向量嵌入",
        "safety": "安全审核",
        "translation": "翻译",
        "vision": "视觉",
        "tooling": "工具/其他",
    }
    for r in rows:
        total = "" if r["total_params_B"] is None else f"{r['total_params_B']:g}"
        active = "" if r["active_params_B"] is None else f"{r['active_params_B']:g}"
        lines.append(
            f"| `{r['model_id']}` | {cat_map.get(r['category'], r['category'])} | {total} | {active} | {tier_map[r['chat_tier']]} | {r['spec_source']} |"
        )

    chat_rows = [r for r in rows if r["chat_tier"] > 0]
    lines += [
        "",
        "## 对话/代码向 — 按档位排序（完整）",
        "",
    ]
    for tier in [5, 4, 3, 2, 1]:
        bucket = [r for r in chat_rows if r["chat_tier"] == tier]
        if not bucket:
            continue
        lines.append(f"### 档位 {tier}（{bucket[0]['chat_tier_label']}）— {len(bucket)} 个")
        lines.append("")
        for r in bucket:
            total = r["total_params_B"]
            active = r["active_params_B"]
            spec = f"总 {total:g}B" if total else "总参未公开"
            if active and active != total:
                spec += f"，激活 {active:g}B"
            elif active:
                spec += f"（稠密）"
            lines.append(f"- `{r['model_id']}` — {spec} — {r['spec_source']}")
        lines.append("")

    unknown = [r for r in rows if r["chat_tier"] == -1]
    if unknown:
        lines.append(f"### 对话向但参数量未公开 — {len(unknown)} 个")
        lines.append("")
        for r in unknown:
            lines.append(f"- `{r['model_id']}`")
        lines.append("")

    non_chat = [r for r in rows if r["chat_tier"] == 0]
    lines.append(f"## 非对话主模型 — {len(non_chat)} 个（不适合当 Codex 主脑）")
    lines.append("")
    for r in non_chat:
        lines.append(f"- `{r['model_id']}`（{cat_map.get(r['category'], r['category'])}）")

    OUT_MD.write_text("\n".join(lines) + "\n", encoding="utf-8")
    print(f"wrote {len(rows)} models -> {OUT_MD.name}")


if __name__ == "__main__":
    main()
