"""SI_1 Engine — now driven by Cactus SI.

Multi-role agents removed. Single agent: Cactus SI.
"""

from __future__ import annotations

from dataclasses import dataclass, field
from datetime import datetime
from typing import Any, Dict, List, Optional

from .agents import AgentRegistry
from .memory import ConversationMemory
from .nlp import Intent, NLPPipeline, NLPResult
from .providers import ChatMessage, CompletionResult, ProviderRouter
from .tools import ToolExecutor, ToolProposal, parse_tool_proposals


@dataclass
class SI1Config:
    ollama_url: str = "http://127.0.0.1:11434"
    ollama_model: str = "cactus-ai:latest"
    gemini_model: str = "gemini-2.0-flash"
    max_tokens: int = 1024
    memory_turns: int = 24
    default_agent: str = "cactus_si"


@dataclass
class SI1Response:
    text: str
    intent: Intent
    language: str
    provider: str
    model: str
    agent: str
    proposals: List[ToolProposal] = field(default_factory=list)
    nlp: Optional[NLPResult] = None
    ok: bool = True
    error: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "text": self.text,
            "intent": self.intent.value,
            "language": self.language,
            "provider": self.provider,
            "model": self.model,
            "agent": self.agent,
            "proposals": [p.to_dict() for p in self.proposals],
            "ok": self.ok,
            "error": self.error,
            "timestamp": datetime.now().isoformat(timespec="seconds"),
        }


class SI1Engine:
    """Cognitive core now centered on Cactus SI."""

    def __init__(self, config: Optional[SI1Config] = None):
        self.config = config or SI1Config()
        self.nlp = NLPPipeline()
        self.router = ProviderRouter(
            ollama_url=self.config.ollama_url,
            ollama_model=self.config.ollama_model,
            gemini_model=self.config.gemini_model,
            max_tokens=self.config.max_tokens,
        )
        self.agents = AgentRegistry()
        self.memory = ConversationMemory(max_turns=self.config.memory_turns)
        self.tools = ToolExecutor()
        self._agent_memory: Dict[str, ConversationMemory] = {}

    def _mem(self, agent_id: str) -> ConversationMemory:
        if agent_id not in self._agent_memory:
            self._agent_memory[agent_id] = ConversationMemory(max_turns=self.config.memory_turns)
        return self._agent_memory[agent_id]

    def chat(
        self,
        user_text: str,
        agent_id: Optional[str] = None,
        prefer_provider: Optional[str] = None,
        use_memory: bool = True,
    ) -> SI1Response:
        agent_id = agent_id or self.config.default_agent
        role = self.agents.get(agent_id)
        nlp = self.nlp.process(user_text)

        if not nlp.safe:
            return SI1Response(
                text="Wykryto niebezpieczny wzorzec w wejściu. Odrzucono.",
                intent=nlp.intent,
                language=nlp.language,
                provider="none",
                model="",
                agent=role.name,
                nlp=nlp,
                ok=False,
                error="unsafe_input",
            )

        system = self.nlp.build_system_prompt(nlp.intent, nlp.language, role.name)
        system = f"{role.system_prompt}\n{system}"

        mem = self._mem(agent_id) if use_memory else ConversationMemory(max_turns=2)
        if use_memory:
            mem.add("user", nlp.text)

        messages = mem.as_list(system=system) if use_memory else [
            ChatMessage(role="system", content=system),
            ChatMessage(role="user", content=nlp.text),
        ]

        if nlp.intent == Intent.COMMAND:
            messages.append(
                ChatMessage(
                    role="system",
                    content=(
                        'If a shell command is needed, include a fenced JSON block: '
                        '`{"command": "...", "rationale": "..."}`. Do not claim you executed it.'
                    ),
                )
            )

        result: CompletionResult = self.router.complete(
            messages, prefer=prefer_provider, nlp=nlp, agent_name=role.name
        )

        if result.ok and use_memory and result.provider not in ("local_nlp", "local_nlp_fallback"):
            mem.add("assistant", result.text)

        proposals = parse_tool_proposals(result.text) if result.ok and result.provider.startswith("ollama") else []

        return SI1Response(
            text=result.text if result.ok else (result.error or "provider_error"),
            intent=nlp.intent,
            language=nlp.language,
            provider=result.provider,
            model=result.model,
            agent=role.name,
            proposals=proposals,
            nlp=nlp,
            ok=result.ok,
            error=result.error,
        )

    def status(self) -> Dict[str, Any]:
        return {
            "engine": "Cactus SI",
            "version": "1.2.0-CactusSI",
            "operator": "Mateusz Mitek",
            "providers": self.router.health(),
            "agents": self.agents.list_roles(),
            "memory_turns": self.config.memory_turns,
            "timestamp": datetime.now().isoformat(timespec="seconds"),
            "note": "Multi-role agents and Cactus AI removed; Cactus SI is primary.",
        }

    def approve_tool(self, proposal: ToolProposal) -> Dict[str, Any]:
        return self.tools.run(proposal, approve=True)
