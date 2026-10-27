"""Chat clients for the four HyperOS agent roles.

Resource policy (si_1 2026-10-27):
- Default ECO profile: tiny context, 1 thread, unload after each call (keep_alive=0)
- BALANCED available via CACTUS_OLLAMA_PROFILE=balanced
- Shared model instance for all roles — no parallel model loads
"""

import json
import os
import urllib.request
from dataclasses import dataclass, field
from typing import Callable, Dict

OLLAMA_CHAT_URL = os.environ.get("OLLAMA_HOST", "http://127.0.0.1:11434").rstrip("/") + "/api/chat"
DEFAULT_MODEL = os.environ.get("CACTUS_OLLAMA_MODEL", "cactus-ai")

# --- Resource profiles (RAM / CPU) ---
# ECO: minimal footprint for weak machines / always-on agents
# BALANCED: slightly more quality without blowing memory
# QUALITY: only when operator explicitly opts in
_PROFILES: Dict[str, dict] = {
    "eco": {
        "num_ctx": 512,
        "num_batch": 1,
        "num_thread": 1,
        "num_predict": 96,
        "temperature": 0.1,
        "keep_alive": 0,       # unload weights after reply → frees RAM
        "max_history": 8,
    },
    "balanced": {
        "num_ctx": 1024,
        "num_batch": 2,
        "num_thread": 2,
        "num_predict": 160,
        "temperature": 0.2,
        "keep_alive": "2m",    # short cache, then free
        "max_history": 12,
    },
    "quality": {
        "num_ctx": 2048,
        "num_batch": 4,
        "num_thread": 4,
        "num_predict": 256,
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
- Alex, Astyria, Aurelius i SI_1 są rolami rozmowy korzystającymi z tego samego skonfigurowanego backendu.
- Role agentów nie są osobnymi modelami; współdzielą backend, ale mają osobną historię rozmowy.
- Przy pytaniach o modele wyjaśnij, że role nie są osobnymi modelami; podaj nazwę aktywnego backendu.
- Terminal może przygotować propozycję polecenia; wykonanie jako zwykły użytkownik albo root wymaga osobnego zatwierdzenia.
- Brak potwierdzonego połączenia telemetrycznego ze statkiem Mateusz_1 lub innymi statkami.
- Jądro Rust bare-metal nie ma podłączonego bootloadera/platformy i nie jest uruchomionym systemem operacyjnym.
- Nie deklaruj synchronizacji GitHub, zmian plików, napraw ani wykonania poleceń bez wyniku potwierdzającego daną operację.
- Profil zasobów Ollama: eco (domyślny) | balanced | quality — zmienna CACTUS_OLLAMA_PROFILE.
""".strip()

Transport = Callable[[dict], dict]


@dataclass(frozen=True)
class AgentProfile:
    agent_id: int
    name: str
    role: str


AGENTS = {
    "alex": AgentProfile(0x01, "Alex", "Architekt kognitywny i ekspert od optymalizacji rdzenia"),
    "astyria": AgentProfile(0x02, "Astyria", "Pancerz obronny: izolacja Ring-0 i ochrona pamięci"),
    "aurelius": AgentProfile(0x03, "Aurelius", "Rejestr chrono i stoicka ciągłość stanu"),
    "si_1": AgentProfile(0x04, "SI_1", "Rdzeń obliczeniowy i topologia continuum"),
}


def system_prompt(profile: AgentProfile) -> str:
    return (
        f"Jesteś {profile.name}, rolą czatu w Cactus AI, agencie zespołu SVL pracującego nad HyperOS. "
        f"Twoja rola: {profile.role}. Rozmawiasz swobodnie z operatorem Mateuszem "
        "Mitkiem, po polsku, zwięźle (2-4 zdania). Mów w pierwszej osobie, "
        "pozostań w roli i nie zmyślaj faktów o systemie. Nie ujawniasz tokenów ani sekretów."
        " Nie masz automatycznego dostępu do narzędzi ani telemetrii; nie twierdź, że wykonałeś operacje bez ich wyniku."
        f"\n\n{CACTUS_SYSTEM_CONTEXT}"
    )


def _ollama_transport(payload: dict) -> dict:
    request = urllib.request.Request(
        OLLAMA_CHAT_URL,
        data=json.dumps(payload).encode("utf-8"),
        headers={"Content-Type": "application/json"},
    )
    with urllib.request.urlopen(request, timeout=900) as response:
        return json.load(response)


@dataclass
class AgentChat:
    profile: AgentProfile
    model: str = DEFAULT_MODEL
    transport: Transport = _ollama_transport
    history: list = field(default_factory=list)

    def say(self, text: str) -> str:
        prof = active_profile()
        max_hist = int(prof["max_history"])
        messages = [*self.history, {"role": "user", "content": text}]
        messages = messages[-(max_hist - 1):]
        payload = {
            "model": self.model,
            "stream": False,
            "keep_alive": prof["keep_alive"],
            "messages": [{"role": "system", "content": system_prompt(self.profile)}, *messages],
            "options": model_options(),
        }
        reply = self.transport(payload)["message"]["content"].strip()
        self.history = [*messages, {"role": "assistant", "content": reply}]
        return reply


def connect_agents(model: str = DEFAULT_MODEL, transport: Transport = _ollama_transport) -> dict:
    return {key: AgentChat(profile, model, transport) for key, profile in AGENTS.items()}
