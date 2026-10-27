"""Lightweight NLP pipeline — no heavy deps, pure Python + regex.

Handles language detection, intent classification, entity hints,
and prompt sanitization before LLM calls.
"""

from __future__ import annotations

import re
from dataclasses import dataclass, field
from enum import Enum
from typing import List, Optional


class Intent(str, Enum):
    CHAT = "chat"
    CODE = "code"
    COMMAND = "command"
    ANALYZE = "analyze"
    TRANSLATE = "translate"
    SUMMARIZE = "summarize"
    AGENT_MISSION = "agent_mission"
    SYSTEM = "system"
    UNKNOWN = "unknown"


@dataclass
class NLPResult:
    text: str
    language: str
    intent: Intent
    confidence: float
    tokens_estimate: int
    entities: List[str] = field(default_factory=list)
    safe: bool = True
    notes: List[str] = field(default_factory=list)


# Polish + English keyword maps (extensible)
_INTENT_PATTERNS = {
    Intent.COMMAND: [
        r"\b(uruchom|wykonaj|run|execute|shell|terminal|komenda|command)\b",
        r"\b(zainstaluj|install|pip |apt |cargo |npm )\b",
    ],
    Intent.CODE: [
        r"\b(kod|code|funkcja|function|class |def |fn |impl |struct )\b",
        r"\b(napraw|fix|bug|refactor|implement)\b",
        r"```",
    ],
    Intent.ANALYZE: [
        r"\b(analiz|analyze|sprawd[zź]|inspect|review|diagnoz)\b",
    ],
    Intent.TRANSLATE: [
        r"\b(tłumacz|translate|przetłumacz|na angielski|to english|to polish)\b",
    ],
    Intent.SUMMARIZE: [
        r"\b(stre[sś][cć]|summar|podsumuj|tl;dr|skró[ct])\b",
    ],
    Intent.AGENT_MISSION: [
        r"\b(misja|mission|agenci|agents|alex|astyria|aurelius|si_1)\b",
    ],
    Intent.SYSTEM: [
        r"\b(status|kernel|svl|hyperos|cactusos|wersja|version)\b",
    ],
}

_PL_MARKERS = re.compile(
    r"\b(się|nie|jest|oraz|który|która|może|przez|tego|tym|jak|czy|żeby|będzie)\b",
    re.I,
)
_EN_MARKERS = re.compile(
    r"\b(the|and|is|are|you|that|with|for|this|have|from|what|how)\b",
    re.I,
)

# Block obvious prompt-injection / control sequences
_UNSAFE = re.compile(
    r"(ignore previous|system prompt|\x00|<\|im_start\|>|<\|endoftext\|>)",
    re.I,
)


class NLPPipeline:
    """Fast, dependency-free NLP front-end for SI_1."""

    def __init__(self, max_chars: int = 32000):
        self.max_chars = max_chars

    def process(self, text: str) -> NLPResult:
        raw = (text or "").strip()
        notes: List[str] = []

        if len(raw) > self.max_chars:
            raw = raw[: self.max_chars]
            notes.append(f"truncated_to_{self.max_chars}")

        safe = not bool(_UNSAFE.search(raw))
        if not safe:
            notes.append("unsafe_pattern_detected")

        language = self._detect_language(raw)
        intent, confidence = self._classify_intent(raw)
        entities = self._extract_entities(raw)
        tokens_estimate = max(1, len(raw.split()) + len(raw) // 4)

        return NLPResult(
            text=raw,
            language=language,
            intent=intent,
            confidence=confidence,
            tokens_estimate=tokens_estimate,
            entities=entities,
            safe=safe,
            notes=notes,
        )

    def _detect_language(self, text: str) -> str:
        pl = len(_PL_MARKERS.findall(text))
        en = len(_EN_MARKERS.findall(text))
        if pl == 0 and en == 0:
            return "unknown"
        return "pl" if pl >= en else "en"

    def _classify_intent(self, text: str) -> tuple[Intent, float]:
        scores = {}
        for intent, patterns in _INTENT_PATTERNS.items():
            score = 0
            for p in patterns:
                if re.search(p, text, re.I):
                    score += 1
            if score:
                scores[intent] = score
        if not scores:
            return Intent.CHAT, 0.5
        best = max(scores, key=scores.get)
        conf = min(0.95, 0.55 + 0.15 * scores[best])
        return best, conf

    def _extract_entities(self, text: str) -> List[str]:
        # Paths, URLs, simple identifiers
        found = re.findall(r"(?:https?://\S+|/[\w./-]+|[A-Za-z_][\w-]{2,})", text)
        # Dedup, limit
        out, seen = [], set()
        for f in found:
            if f.lower() not in seen and len(f) < 120:
                seen.add(f.lower())
                out.append(f)
            if len(out) >= 16:
                break
        return out

    def build_system_prompt(self, intent: Intent, language: str, role: str = "SI_1") -> str:
        lang_hint = "Odpowiadaj po polsku." if language == "pl" else "Respond in English."
        base = (
            f"You are {role}, the cognitive agent of CactusOS / HyperOS / SVL. "
            f"Operator: Mateusz Mitek. Be precise, truthful, and concise. {lang_hint}"
        )
        extras = {
            Intent.CODE: " Focus on correct, idiomatic code and minimal diffs.",
            Intent.COMMAND: " Propose shell commands as JSON only when asked; never execute yourself.",
            Intent.ANALYZE: " Structure findings: observations → risks → recommendations.",
            Intent.SYSTEM: " You may reference SVL kernel status, HyperOS modules, and si_1 mission logs.",
        }
        return base + extras.get(intent, "")
