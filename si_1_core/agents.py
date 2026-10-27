"""Single agent: Cactus AI (Ollama).

Cactus SI and multi-role personas removed.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict


@dataclass(frozen=True)
class AgentRole:
    id: str
    name: str
    system_prompt: str
    temperature_hint: float = 0.2


class AgentRegistry:
    def __init__(self):
        self._roles: Dict[str, AgentRole] = {}
        self.register(
            AgentRole(
                id="cactus_ai",
                name="Cactus AI",
                system_prompt=(
                    "You are Cactus AI, the local Ollama model of CactusOS. "
                    "Respond in Polish when the user writes in Polish. "
                    "Be concise and precise. Do not claim command execution."
                ),
            )
        )

    def register(self, role: AgentRole) -> None:
        self._roles[role.id] = role

    def get(self, role_id: str) -> AgentRole:
        return self._roles.get(role_id) or self._roles["cactus_ai"]

    def list_roles(self) -> Dict[str, str]:
        return {k: v.name for k, v in self._roles.items()}
