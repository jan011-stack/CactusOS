"""Pure local NLP responses — no Ollama, no API, no network.

Used when CACTUS_LOCAL_ONLY=1 or when all generative providers fail.
Answers are template-based from intent + entities (from nlp.py).
"""

from __future__ import annotations

from .nlp import Intent, NLPResult


def local_reply(nlp: NLPResult, agent_name: str = "SI_1") -> str:
    """Generate a short, deterministic reply from NLP result only."""
    lang = nlp.language
    intent = nlp.intent
    entities = ", ".join(nlp.entities[:5]) if nlp.entities else "(brak)"

    if lang == "pl":
        base = {
            Intent.SYSTEM: f"[{agent_name} lokalny] Status systemu: NLP aktywne, backend generatywny niepodłączony. Compact metric i kernel SVL dostępne lokalnie.",
            Intent.COMMAND: f"[{agent_name} lokalny] Wykryto intencję polecenia. Encje: {entities}. Propozycja komendy wymaga zatwierdzenia (WYKONAJ). Nie wykonuję automatycznie.",
            Intent.CODE: f"[{agent_name} lokalny] Tryb kodu. Encje/pliki: {entities}. Pełne generowanie wymaga Ollamy lub API. Mogę tylko sklasyfikować.",
            Intent.ANALYZE: f"[{agent_name} lokalny] Analiza (NLP): intencja={intent.value}, pewność={nlp.confidence:.2f}, język={lang}. Encje: {entities}.",
            Intent.AGENT_MISSION: f"[{agent_name} lokalny] Misja agentów (SI_1/Alex/Astyria/Aurelius) — role podłączone. Generatywne odpowiedzi wymagają backendu.",
            Intent.TRANSLATE: f"[{agent_name} lokalny] Tłumaczenie offline ograniczone. Tekst wykryty jako {lang}.",
            Intent.SUMMARIZE: f"[{agent_name} lokalny] Podsumowanie NLP: {nlp.tokens_estimate} tokenów szacowanych, intencja={intent.value}.",
        }
        return base.get(intent, f"[{agent_name} lokalny] Czat NLP. Intencja={intent.value}, pewność={nlp.confidence:.2f}. Encje: {entities}. Ustaw Ollamę lub API dla pełnych odpowiedzi.")
    else:
        base = {
            Intent.SYSTEM: f"[{agent_name} local] System status: NLP active, generative backend not connected. SVL kernel available offline.",
            Intent.COMMAND: f"[{agent_name} local] Command intent detected. Entities: {entities}. Proposal requires explicit approval. I do not auto-execute.",
            Intent.CODE: f"[{agent_name} local] Code mode. Entities: {entities}. Full generation needs Ollama or API.",
            Intent.ANALYZE: f"[{agent_name} local] NLP analysis: intent={intent.value}, confidence={nlp.confidence:.2f}, lang={lang}. Entities: {entities}.",
            Intent.AGENT_MISSION: f"[{agent_name} local] Agent mission roles bound. Generative replies need a backend.",
        }
        return base.get(intent, f"[{agent_name} local] NLP chat. Intent={intent.value}, confidence={nlp.confidence:.2f}. Entities: {entities}. Set Ollama or API for full replies.")
