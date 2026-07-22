#!/usr/bin/env python3
import json
import pathlib
import re
import urllib.request

ROOT = pathlib.Path(__file__).resolve().parents[1]
COCKPIT = pathlib.Path.home() / ".antigravity_cockpit"
ACCOUNT_PATH = COCKPIT / "codex_accounts/codex_apikey_744ece4e421cb34fbe11f5a88bc06884.json"
PROVIDERS_PATH = COCKPIT / "codex_model_providers.json"
MANIFEST_PATH = COCKPIT / (
    "codex_provider_gateway_sidecars/"
    "7f0b48b307ebc2d71fcfebc21948c248ded030d3875462dc45e75f58b3d1e4e6/manifest.json"
)
PRESET_PATH = ROOT / "src/utils/codexProviderPresets.ts"
MODELS_JSON = ROOT / "scripts/_nvidia_models_full.json"


def fetch_models(api_key: str) -> list[str]:
    req = urllib.request.Request(
        "https://integrate.api.nvidia.com/v1/models",
        headers={"Authorization": f"Bearer {api_key}"},
    )
    with urllib.request.urlopen(req, timeout=30) as response:
        payload = json.loads(response.read())
    return sorted(item["id"] for item in payload["data"])


def patch_preset(models: list[str]) -> None:
    lines = ["    modelCatalog: ["]
    for model in models:
        lines.append(f'      "{model}",')
    lines.append("    ],")
    block = "\n".join(lines)
    text = PRESET_PATH.read_text(encoding="utf-8")
    pattern = (
        r'(\{\s*\n\s*id: "nvidia",\s*\n\s*name: "Nvidia",\s*\n'
        r'\s*baseUrls: \["https://integrate\.api\.nvidia\.com/v1"\],\s*\n)(\s*website:)'
    )
    new_text, count = re.subn(pattern, r"\1" + block + "\n\2", text, count=1)
    if count != 1:
        raise RuntimeError(f"preset patch failed: {count}")
    PRESET_PATH.write_text(new_text, encoding="utf-8")


def main() -> None:
    account = json.loads(ACCOUNT_PATH.read_text(encoding="utf-8"))
    models = fetch_models(account["openai_api_key"])
    MODELS_JSON.write_text(json.dumps(models, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    account["api_model_catalog"] = models
    account["api_wire_api"] = "chat_completions"
    account["api_provider_id"] = "cmp_1782107121425_2"
    account["api_provider_name"] = "NVIDIA API"
    ACCOUNT_PATH.write_text(json.dumps(account, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    providers = json.loads(PROVIDERS_PATH.read_text(encoding="utf-8"))
    for provider in providers:
        if provider.get("id") == "cmp_1782107121425_2":
            provider["modelCatalog"] = models
            provider["wireApi"] = "chat_completions"
            provider["enableModePreference"] = "gateway"
            provider["name"] = "NVIDIA API"
        if provider.get("id") == "cmp_1782107025348_1":
            provider["baseUrl"] = "https://api.openai.com/v1"
    PROVIDERS_PATH.write_text(json.dumps(providers, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    manifest = json.loads(MANIFEST_PATH.read_text(encoding="utf-8"))
    for api_key in manifest.get("apiKeys", []):
        gateway = api_key.get("providerGateway") or {}
        if "integrate.api.nvidia.com" in gateway.get("baseUrl", ""):
            gateway["upstreamModel"] = "deepseek-ai/deepseek-v4-flash"
            gateway["upstreamModels"] = models
            gateway["wireApi"] = "chat_completions"
    MANIFEST_PATH.write_text(json.dumps(manifest, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    patch_preset(models)
    print(f"patched {len(models)} models")


if __name__ == "__main__":
    main()
