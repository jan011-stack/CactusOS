"""Configure a local SI_1 chat model without replacing other VS Code providers."""

import argparse
import json
import os
import platform
import shutil
import tempfile
from pathlib import Path
from urllib.parse import urlsplit


def default_config_file():
    system = platform.system()
    if system == "Windows":
        return Path(os.environ["APPDATA"]) / "Code/User/chatLanguageModels.json"
    if system == "Darwin":
        return Path.home() / "Library/Application Support/Code/User/chatLanguageModels.json"
    return Path(os.environ.get("XDG_CONFIG_HOME", Path.home() / ".config")) / "Code/User/chatLanguageModels.json"


def inject_si1_to_vscode(config_file, model="cactus-ai:latest",
                         url="http://127.0.0.1:11434/v1/chat/completions",
                         tool_calling=False, max_input_tokens=512, max_output_tokens=160):
    endpoint = urlsplit(url)
    if (endpoint.scheme not in {"http", "https"} or not endpoint.hostname
            or endpoint.username is not None or endpoint.password is not None
            or endpoint.port == 0 or any(character.isspace() for character in url)
            or endpoint.query or endpoint.fragment
            or not endpoint.path.endswith("/chat/completions")):
        raise ValueError("Podaj URL HTTP(S) konczacy sie /chat/completions, bez sekretow")
    if not model.strip() or max_input_tokens <= 0 or max_output_tokens <= 0:
        raise ValueError("Model i dodatnie limity tokenow sa wymagane")
    config_file = Path(config_file).expanduser()
    providers = json.loads(config_file.read_text(encoding="utf-8")) if config_file.exists() else []
    if not isinstance(providers, list) or not all(isinstance(item, dict) for item in providers):
        raise ValueError("Konfiguracja musi byc tablica obiektow JSON; plik pozostaje bez zmian")
    matches = [item for item in providers if item.get("vendor") == "customendpoint" and item.get("name") == "SI_1"]
    if len(matches) > 1:
        raise ValueError("Zduplikowana grupa SI_1; wybierz grupe recznie w VS Code")
    before = json.dumps(providers, sort_keys=True)
    group = matches[0] if matches else {"name": "SI_1", "vendor": "customendpoint", "models": []}
    models = group.get("models", [])
    if not isinstance(models, list) or not all(isinstance(item, dict) for item in models):
        raise ValueError("Pole models musi byc tablica obiektow JSON")
    matching_models = [item for item in models if item.get("id") == model]
    if len(matching_models) > 1:
        raise ValueError("Zduplikowany identyfikator modelu SI_1")
    entry = matching_models[0] if matching_models else {}
    entry.update({
        "id": model, "name": "SI_1 (Cactus AI)", "apiType": "chat-completions",
        "url": url, "toolCalling": tool_calling, "vision": False,
        "maxInputTokens": max_input_tokens, "maxOutputTokens": max_output_tokens,
    })
    if not matching_models:
        models.append(entry)
    group["models"] = models
    if not matches:
        providers.append(group)
    if before == json.dumps(providers, sort_keys=True):
        return config_file
    config_file.parent.mkdir(parents=True, exist_ok=True)
    if config_file.exists():
        backup = config_file.with_suffix(config_file.suffix + ".bak")
        if not backup.exists():
            shutil.copy2(config_file, backup)
    temporary_path = None
    try:
        with tempfile.NamedTemporaryFile(mode="w", encoding="utf-8", dir=config_file.parent,
                                         prefix=config_file.name + ".", delete=False) as temporary:
            temporary_path = Path(temporary.name)
            json.dump(providers, temporary, indent=2, ensure_ascii=False)
            temporary.write("\n")
        os.replace(temporary_path, config_file)
    finally:
        if temporary_path is not None:
            temporary_path.unlink(missing_ok=True)
    return config_file


def main(argv=None):
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--config-file", type=Path, help="Plik modeli aktywnego profilu VS Code")
    parser.add_argument("--model", default="cactus-ai:latest")
    parser.add_argument("--url", default="http://127.0.0.1:11434/v1/chat/completions")
    parser.add_argument("--tool-calling", action="store_true", help="Tylko dla modelu z potwierdzona obsluga narzedzi")
    parser.add_argument("--max-input-tokens", type=int, default=512)
    parser.add_argument("--max-output-tokens", type=int, default=160)
    args = parser.parse_args(argv)
    if args.config_file is None and (os.environ.get("CODESPACES") or os.environ.get("VSCODE_AGENT_FOLDER")
                                     or os.environ.get("REMOTE_CONTAINERS")):
        parser.error("W srodowisku zdalnym wskaz --config-file aktywnego profilu VS Code")
    try:
        path = inject_si1_to_vscode(
            args.config_file or default_config_file(), args.model, args.url,
            args.tool_calling, args.max_input_tokens, args.max_output_tokens,
        )
    except (OSError, ValueError, KeyError) as error:
        parser.exit(1, f"Blad konfiguracji: {error}\n")
    print(f"Zapisano konfiguracje SI_1: {path}")
    print("To konfiguracja modelu, nie potwierdzenie polaczenia ani wykonania zadania.")


if __name__ == "__main__":
    main()