"""Local Ollama model resource profiles.

Resource policy:
- Default ECO profile: small context, all cores, model kept loaded for 5 minutes
- BALANCED / QUALITY available via CACTUS_OLLAMA_PROFILE
"""

import os
from typing import Dict

DEFAULT_MODEL = os.environ.get("CACTUS_OLLAMA_MODEL", "llama3.1:8b")
VISION_MODEL = os.environ.get("CACTUS_OLLAMA_VISION_MODEL", "llama3.2-vision")

# --- Resource profiles (RAM / CPU) ---
# ECO: minimal footprint for weak machines
# BALANCED: slightly more quality without blowing memory
# QUALITY: only when operator explicitly opts in
_PROFILES: Dict[str, dict] = {
    "eco": {
        "num_ctx": 2048,
        "num_batch": 64,
        "num_thread": 4,
        "num_predict": 1024,
        "temperature": 0.1,
        "keep_alive": "5m",
        "max_history": 8,
    },
    "balanced": {
        "num_ctx": 3072,
        "num_batch": 128,
        "num_thread": 4,
        "num_predict": 1536,
        "temperature": 0.2,
        "keep_alive": "2m",    # short cache, then free
        "max_history": 12,
    },
    "quality": {
        "num_ctx": 4096,
        "num_batch": 256,
        "num_thread": 4,
        "num_predict": 2048,
        "temperature": 0.3,
        "keep_alive": "5m",
        "max_history": 16,
    },
}


def _active_profile_name() -> str:
    name = os.environ.get("CACTUS_OLLAMA_PROFILE", "eco").strip().lower()
    return name if name in _PROFILES else "eco"


def active_profile() -> dict:
    return _PROFILES[_active_profile_name()].copy()


def model_options() -> dict:
    p = active_profile()
    return {
        "num_ctx": p["num_ctx"],
        "num_batch": p["num_batch"],
        "num_thread": p["num_thread"],
        "num_predict": p["num_predict"],
        "temperature": p["temperature"],
    }


# Backward-compatible names used by tests / web_server
MODEL_OPTIONS = model_options()
MAX_HISTORY_MESSAGES = active_profile()["max_history"]

CACTUS_SYSTEM_CONTEXT = """
Zweryfikowane fakty o środowisku CactusOS:
- Gemini 3.1 Flash-Lite jest głównym modelem terminala webowego, a Ollama jest jego fallbackiem.
- Terminal może przygotować propozycję polecenia; wykonanie jako zwykły użytkownik albo root wymaga osobnego zatwierdzenia.
- Brak potwierdzonego połączenia telemetrycznego ze statkiem Mateusz_1 lub innymi statkami.
- Jądro Rust bare-metal nie ma podłączonego bootloadera/platformy i nie jest uruchomionym systemem operacyjnym.
- Nie deklaruj synchronizacji GitHub, zmian plików, napraw ani wykonania poleceń bez wyniku potwierdzającego daną operację.
- Profil zasobów Ollama: eco (domyślny) | balanced | quality — zmienna CACTUS_OLLAMA_PROFILE.
""".strip()
