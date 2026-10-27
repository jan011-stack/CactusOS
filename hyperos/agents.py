"""Chat clients for the four HyperOS agent roles."""

import json
import urllib.request
from dataclasses import dataclass, field
from typing import Callable

OLLAMA_CHAT_URL = "http://127.0.0.1:11434/api/chat"
DEFAULT_MODEL = "cactus-ai"
MODEL_OPTIONS = {
    "num_ctx": 512,
    "num_batch": 1,
    "num_thread": 1,
    "num_predict": 96,
    "temperature": 0.1,
}
MAX_HISTORY_MESSAGES = 12

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
        messages = [*self.history, {"role": "user", "content": text}]
        messages = messages[-(MAX_HISTORY_MESSAGES - 1):]
        payload = {
            "model": self.model,
            "stream": False,
            "keep_alive": 0,
            "messages": [{"role": "system", "content": system_prompt(self.profile)}, *messages],
            "options": MODEL_OPTIONS.copy(),
        }
        reply = self.transport(payload)["message"]["content"].strip()
        self.history = [*messages, {"role": "assistant", "content": reply}]
        return reply


def connect_agents(model: str = DEFAULT_MODEL, transport: Transport = _ollama_transport) -> dict:
    return {key: AgentChat(profile, model, transport) for key, profile in AGENTS.items()}
