#!/usr/bin/env python3
"""Restore dual NVIDIA accounts in Cockpit index + providers."""
import json
import pathlib
import time

COCKPIT = pathlib.Path.home() / ".antigravity_cockpit"
ACCOUNTS_DIR = COCKPIT / "codex_accounts"
INDEX_PATH = COCKPIT / "codex_accounts.json"
PROVIDERS_PATH = COCKPIT / "codex_model_providers.json"
MODELS_JSON = pathlib.Path(__file__).resolve().parents[1] / "scripts/_nvidia_models_full.json"

OLD_ID = "codex_apikey_744ece4e421cb34fbe11f5a88bc06884"
NEW_ID = "codex_apikey_fd0694cfc693acf251898c5cf171f242"
OLD_PROVIDER_ID = "cmp_1782107121425_2"
NEW_PROVIDER_ID = "cmp_1782113361000_self"


def load_json(path: pathlib.Path):
    return json.loads(path.read_text(encoding="utf-8"))


def write_json(path: pathlib.Path, data) -> None:
    path.write_text(json.dumps(data, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")


def account_files() -> list[pathlib.Path]:
    return sorted(ACCOUNTS_DIR.glob("codex_apikey_*.json"))


def rebuild_index() -> None:
    summaries = []
    for path in account_files():
        account = load_json(path)
        created_at = account.get("created_at", int(time.time()))
        last_used = account.get("last_used") or created_at
        summaries.append(
            {
                "id": account["id"],
                "email": account["email"],
                "plan_type": account.get("plan_type", "API_KEY"),
                "created_at": created_at,
                "last_used": last_used,
            }
        )
    summaries.sort(key=lambda item: (item.get("last_used") or 0, item.get("created_at") or 0), reverse=True)
    index = load_json(INDEX_PATH) if INDEX_PATH.exists() else {"version": "1.0", "accounts": [], "current_account_id": None}
    index["accounts"] = summaries
    if index.get("current_account_id") not in {item["id"] for item in summaries}:
        index["current_account_id"] = NEW_ID
    write_json(INDEX_PATH, index)


def patch_accounts(models: list[str]) -> None:
    old_path = ACCOUNTS_DIR / f"{OLD_ID}.json"
    new_path = ACCOUNTS_DIR / f"{NEW_ID}.json"
    old = load_json(old_path)
    old["account_name"] = "NVIDIA API (外部)"
    old["api_provider_name"] = "NVIDIA API (外部)"
    old["api_provider_id"] = OLD_PROVIDER_ID
    old["api_model_catalog"] = models
    old["api_wire_api"] = "chat_completions"
    write_json(old_path, old)

    if new_path.exists():
        new = load_json(new_path)
    else:
        raise SystemExit(f"missing {new_path}")
    new["account_name"] = "NVIDIA API (自建)"
    new["api_provider_name"] = "NVIDIA API (自建)"
    new["api_provider_id"] = NEW_PROVIDER_ID
    new["api_model_catalog"] = models
    new["api_wire_api"] = "chat_completions"
    if not new.get("last_used"):
        new["last_used"] = new.get("created_at", int(time.time()))
    write_json(new_path, new)


def patch_providers(models: list[str], old_key: str, new_key: str) -> None:
    providers = load_json(PROVIDERS_PATH)
    kept = []
    for provider in providers:
        if provider.get("id") == "cmp_1782107025348_1":
            provider["baseUrl"] = "https://api.openai.com/v1"
        if provider.get("id") in {OLD_PROVIDER_ID, NEW_PROVIDER_ID}:
            continue
        kept.append(provider)

    now_ms = int(time.time() * 1000)
    kept.append(
        {
            "id": OLD_PROVIDER_ID,
            "name": "NVIDIA API (外部)",
            "baseUrl": "https://integrate.api.nvidia.com/v1",
            "modelCatalog": models,
            "supportsVision": False,
            "wireApi": "chat_completions",
            "enableModePreference": "gateway",
            "apiKeys": [
                {
                    "id": "cmk_1782107121425_2",
                    "name": "NVIDIA API (外部)",
                    "apiKey": old_key,
                    "createdAt": 1782107121425,
                    "updatedAt": now_ms,
                }
            ],
            "createdAt": 1782107121425,
            "updatedAt": now_ms,
        }
    )
    kept.append(
        {
            "id": NEW_PROVIDER_ID,
            "name": "NVIDIA API (自建)",
            "baseUrl": "https://integrate.api.nvidia.com/v1",
            "modelCatalog": models,
            "supportsVision": False,
            "wireApi": "chat_completions",
            "enableModePreference": "gateway",
            "apiKeys": [
                {
                    "id": "cmk_1782113361000_self",
                    "name": "NVIDIA API (自建)",
                    "apiKey": new_key,
                    "createdAt": now_ms,
                    "updatedAt": now_ms,
                }
            ],
            "createdAt": now_ms,
            "updatedAt": now_ms,
        }
    )
    write_json(PROVIDERS_PATH, kept)


def main() -> None:
    models = load_json(MODELS_JSON)
    old = load_json(ACCOUNTS_DIR / f"{OLD_ID}.json")
    new = load_json(ACCOUNTS_DIR / f"{NEW_ID}.json")
    patch_accounts(models)
    patch_providers(models, old["openai_api_key"], new["openai_api_key"])
    rebuild_index()
    index = load_json(INDEX_PATH)
    ids = [item["id"] for item in index["accounts"]]
    print("index accounts:", len(ids))
    print("has old:", OLD_ID in ids)
    print("has new:", NEW_ID in ids)
    print("providers:", [p["name"] for p in load_json(PROVIDERS_PATH) if "NVIDIA" in p.get("name", "")])


if __name__ == "__main__":
    main()
