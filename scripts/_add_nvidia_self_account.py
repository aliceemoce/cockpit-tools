#!/usr/bin/env python3
"""Add a second NVIDIA API key account to Cockpit without replacing existing ones."""
import hashlib
import json
import pathlib
import time

COCKPIT = pathlib.Path.home() / ".antigravity_cockpit"
INDEX_PATH = COCKPIT / "codex_accounts.json"
PROVIDERS_PATH = COCKPIT / "codex_model_providers.json"
MODELS_JSON = pathlib.Path(__file__).resolve().parents[1] / "scripts/_nvidia_models_full.json"

NEW_KEY = "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"
ACCOUNT_NAME = "NVIDIA API (自建)"
PROVIDER_NAME = "NVIDIA API (自建)"


def account_id_for_key(api_key: str) -> str:
    return f"codex_apikey_{hashlib.md5(api_key.encode()).hexdigest()}"


def email_for_key(api_key: str) -> str:
    digest = hashlib.md5(api_key.encode()).hexdigest()
    return f"api-key-{digest[:8]}"


def main() -> None:
    models = json.loads(MODELS_JSON.read_text(encoding="utf-8"))
    now = int(time.time())
    account_id = account_id_for_key(NEW_KEY)
    provider_id = f"cmp_{now * 1000}_self"

    account = {
        "id": account_id,
        "email": email_for_key(NEW_KEY),
        "auth_mode": "apikey",
        "openai_api_key": NEW_KEY,
        "api_base_url": "https://integrate.api.nvidia.com/v1",
        "api_provider_mode": "custom",
        "api_provider_id": provider_id,
        "api_provider_name": PROVIDER_NAME,
        "api_model_catalog": models,
        "api_wire_api": "chat_completions",
        "api_supports_vision": False,
        "user_id": None,
        "plan_type": "API_KEY",
        "account_id": None,
        "organization_id": None,
        "account_name": ACCOUNT_NAME,
        "app_speed": "standard",
        "tokens": {"id_token": "", "access_token": ""},
        "token_generation": 0,
        "token_updated_at": now,
        "token_source_mode": "managed",
        "quota": None,
        "tags": None,
        "created_at": now,
        "last_used": None,
    }

    account_path = COCKPIT / "codex_accounts" / f"{account_id}.json"
    account_path.write_text(json.dumps(account, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    index = json.loads(INDEX_PATH.read_text(encoding="utf-8"))
    if not any(item.get("id") == account_id for item in index.get("accounts", [])):
        index.setdefault("accounts", []).append(
            {
                "id": account_id,
                "email": account["email"],
                "plan_type": "API_KEY",
                "created_at": now,
                "last_used": None,
            }
        )
    INDEX_PATH.write_text(json.dumps(index, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    providers = json.loads(PROVIDERS_PATH.read_text(encoding="utf-8"))
    if not any(item.get("id") == provider_id for item in providers):
        providers.append(
            {
                "id": provider_id,
                "name": PROVIDER_NAME,
                "baseUrl": "https://integrate.api.nvidia.com/v1",
                "modelCatalog": models,
                "supportsVision": False,
                "wireApi": "chat_completions",
                "enableModePreference": "gateway",
                "apiKeys": [
                    {
                        "id": f"cmk_{now * 1000}_self",
                        "name": ACCOUNT_NAME,
                        "apiKey": NEW_KEY,
                        "createdAt": now * 1000,
                        "updatedAt": now * 1000,
                    }
                ],
                "createdAt": now * 1000,
                "updatedAt": now * 1000,
            }
        )
    PROVIDERS_PATH.write_text(json.dumps(providers, ensure_ascii=False, indent=2) + "\n", encoding="utf-8")

    print(f"added account {account_id}")
    print(f"provider {provider_id}")


if __name__ == "__main__":
    main()
