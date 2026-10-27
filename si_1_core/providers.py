"""LLM provider router: Ollama (local) → Gemini → optional Cactus SI.

Respects CACTUS_OLLAMA_PROFILE for low RAM/CPU defaults.
"""

from __future__ import annotations

import json
import os
import urllib.request
from dataclasses import dataclass
from typing import Any, Dict, List, Optional


@dataclass
class ChatMessage:
    role: str
    content: str


@dataclass
class CompletionResult:
    text: str
    provider: str
    model: str
    ok: bool
    error: Optional[str] = None
    raw: Optional[Dict[str, Any]] = None


def _ollama_options() -> dict:
    profile = os.environ.get("CACTUS_OLLAMA_PROFILE", "eco").lower()
    profiles = {
        "eco": {"num_ctx": 512, "num_batch": 1, "num_thread": 1, "num_predict": 96, "temperature": 0.1},
        "balanced": {"num_ctx": 1024, "num_batch": 2, "num_thread": 2, "num_predict": 160, "temperature": 0.2},
        "quality": {"num_ctx": 2048, "num_batch": 4, "num_thread": 4, "num_predict": 256, "temperature": 0.3},
    }
    return profiles.get(profile, profiles["eco"]).copy()


def _keep_alive():
    profile = os.environ.get("CACTUS_OLLAMA_PROFILE", "eco").lower()
    return {"eco": 0, "balanced": "2m", "quality": "5m"}.get(profile, 0)


class ProviderRouter:
    def __init__(
        self,
        ollama_url: str = "http://127.0.0.1:11434",
        ollama_model: Optional[str] = None,
        gemini_model: str = "gemini-2.0-flash",
        timeout: float = 60.0,
        max_tokens: int = 1024,
    ):
        self.ollama_url = ollama_url.rstrip("/")
        self.ollama_model = ollama_model or os.environ.get("CACTUS_OLLAMA_MODEL", "cactus-ai:latest")
        self.gemini_model = gemini_model
        self.timeout = timeout
        self.max_tokens = max_tokens
        self.gemini_key = os.environ.get("GEMINI_API_KEY", "").strip()
        self.cactus_url = os.environ.get("CACTUS_SI_API_URL", "").strip().rstrip("/")
        self.cactus_key = os.environ.get("CACTUS_SI_API_KEY", "").strip()

    def complete(self, messages: List[ChatMessage], prefer: Optional[str] = None) -> CompletionResult:
        order = self._order(prefer)
        last_err = None
        for name in order:
            try:
                if name == "cactus_si":
                    return self._cactus_si(messages)
                if name == "gemini":
                    return self._gemini(messages)
                if name == "ollama":
                    return self._ollama(messages)
            except Exception as e:
                last_err = str(e)
                continue
        return CompletionResult(text="", provider="none", model="", ok=False, error=last_err or "no_provider_available")

    def _order(self, prefer: Optional[str]) -> List[str]:
        default = []
        if self.cactus_url and self.cactus_key:
            default.append("cactus_si")
        if self.gemini_key:
            default.append("gemini")
        default.append("ollama")
        if prefer and prefer in default:
            return [prefer] + [x for x in default if x != prefer]
        return default

    def _post_json(self, url: str, body: dict, headers: Optional[dict] = None) -> dict:
        data = json.dumps(body).encode("utf-8")
        req = urllib.request.Request(
            url, data=data,
            headers={**(headers or {}), "Content-Type": "application/json"},
            method="POST",
        )
        with urllib.request.urlopen(req, timeout=self.timeout) as resp:
            return json.loads(resp.read().decode("utf-8"))

    def _ollama(self, messages: List[ChatMessage]) -> CompletionResult:
        opts = _ollama_options()
        opts["num_predict"] = min(opts.get("num_predict", 96), self.max_tokens)
        body = {
            "model": self.ollama_model,
            "messages": [{"role": m.role, "content": m.content} for m in messages],
            "stream": False,
            "keep_alive": _keep_alive(),
            "options": opts,
        }
        raw = self._post_json(f"{self.ollama_url}/api/chat", body)
        text = (raw.get("message") or {}).get("content") or ""
        return CompletionResult(text=text, provider="ollama", model=self.ollama_model, ok=True, raw=raw)

    def _gemini(self, messages: List[ChatMessage]) -> CompletionResult:
        system_parts = [m.content for m in messages if m.role == "system"]
        contents = []
        for m in messages:
            if m.role == "system":
                continue
            role = "user" if m.role == "user" else "model"
            contents.append({"role": role, "parts": [{"text": m.content}]})
        body: Dict[str, Any] = {
            "contents": contents,
            "generationConfig": {"maxOutputTokens": self.max_tokens, "temperature": 0.7},
        }
        if system_parts:
            body["systemInstruction"] = {"parts": [{"text": "\n".join(system_parts)}]}
        url = (
            f"https://generativelanguage.googleapis.com/v1beta/models/"
            f"{self.gemini_model}:generateContent?key={self.gemini_key}"
        )
        raw = self._post_json(url, body)
        text = ""
        try:
            text = raw["candidates"][0]["content"]["parts"][0]["text"]
        except (KeyError, IndexError, TypeError):
            text = json.dumps(raw)[:500]
        return CompletionResult(text=text, provider="gemini", model=self.gemini_model, ok=True, raw=raw)

    def _cactus_si(self, messages: List[ChatMessage]) -> CompletionResult:
        body = {
            "model": "cactus-si",
            "messages": [{"role": m.role, "content": m.content} for m in messages],
            "max_tokens": self.max_tokens,
        }
        headers = {"Authorization": f"Bearer {self.cactus_key}"}
        url = f"{self.cactus_url}/v1/chat/completions"
        raw = self._post_json(url, body, headers=headers)
        text = ""
        try:
            text = raw["choices"][0]["message"]["content"]
        except (KeyError, IndexError, TypeError):
            text = str(raw)[:500]
        return CompletionResult(text=text, provider="cactus_si", model="cactus-si", ok=True, raw=raw)

    def health(self) -> Dict[str, Any]:
        return {
            "ollama_url": self.ollama_url,
            "ollama_model": self.ollama_model,
            "ollama_profile": os.environ.get("CACTUS_OLLAMA_PROFILE", "eco"),
            "ollama_options": _ollama_options(),
            "keep_alive": _keep_alive(),
            "gemini_configured": bool(self.gemini_key),
            "cactus_si_configured": bool(self.cactus_url and self.cactus_key),
        }
