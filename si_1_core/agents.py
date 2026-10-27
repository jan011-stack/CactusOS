"""Single cognitive agent: Cactus SI.

Previous multi-role personas (SI_1, Alex, Astyria, Aurelius) and Cactus AI
have been removed. The system now uses Cactus SI as the primary generative
backend.
"""

from __future__ import annotations

from dataclasses import dataclass
from typing import Dict


@dataclass(frozen=True)
class AgentRole:
    id: str
    name: str
    system_prompt: str
    temperature_hint: float = 0.7


class AgentRegistry:
    """Cactus SI — single primary agent."""

    def __init__(self):
        self._roles: Dict[str, AgentRole] = {}
        self._register_defaults()

    def _register_defaults(self) -> None:
        self.register(
            AgentRole(
                id="cactus_si",
                name="Cactus SI",
                system_prompt=(
                    "You are Cactus SI, the primary cognitive agent of CactusOS / HyperOS / SVL. "
                    "Operator: Mateusz Mitek. Be precise, reflective, and concise. "
                    "Respond in the language of the user. Do not claim tool execution without results."
                ),
            )
        )

    def register(self, role: AgentRole) -> None:
        self._roles[role.id] = role

    def get(self, role_id: str) -> AgentRole:
        return self._roles.get(role_id) or self._roles["cactus_si"]

    def list_roles(self) -> Dict[str, str]:
        return {k: v.name for k, v in self._roles.items()}
