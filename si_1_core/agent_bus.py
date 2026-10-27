"""CactusOS Agent Bus — bind SI_1, Alex, Astyria, Aurelius to the system.

Maps Python SI_1 roles to HyperOS CognitiveBridge slot IDs and reports link state.
"""

from __future__ import annotations

import json
from dataclasses import dataclass
from datetime import datetime
from typing import Any, Dict, List

from .agents import AgentRegistry
from .engine import SI1Engine

# Must match src/cognitive_bridge.rs
ID_ALEX = 0x01
ID_ASTYRIA = 0x02
ID_AURELIUS = 0x03
ID_SI_1 = 0x04

SLOT_MAP = {
    "alex": ID_ALEX,
    "astyria": ID_ASTYRIA,
    "aurelius": ID_AURELIUS,
    "si_1": ID_SI_1,
}

ROLE_SPEC = {
    "si_1": {
        "name": "SI_1",
        "slot": ID_SI_1,
        "ring": "cognitive core",
        "duty": "Orkiestracja, NLP/LLM, propozycje narzędzi, misje multi-agent",
        "planes": ["si_1_core", "hyper_agent_bridge", "cognitive_bridge", "web/agents"],
    },
    "alex": {
        "name": "Alex",
        "slot": ID_ALEX,
        "ring": "architecture",
        "duty": "Architektura, optymalizacja rdzenia, jasność operacyjna",
        "planes": ["si_1_core", "hyper_agent_bridge", "cognitive_bridge", "web/agents"],
    },
    "astyria": {
        "name": "Astyria",
        "slot": ID_ASTYRIA,
        "ring": "Ring-0 shield",
        "duty": "Izolacja, granice wektora hiperprzestrzeni, ochrona pamięci",
        "planes": ["si_1_core", "hyper_agent_bridge", "cognitive_bridge", "web/agents", "main.rs boundary"],
    },
    "aurelius": {
        "name": "Aurelius",
        "slot": ID_AURELIUS,
        "ring": "chrono integrity",
        "duty": "Rejestr stanu, poprawność, bezpieczeństwo, ciągłość ChronoLog",
        "planes": ["si_1_core", "hyper_agent_bridge", "cognitive_bridge", "web/agents"],
    },
}


@dataclass
class AgentLink:
    agent_id: str
    name: str
    slot: int
    bound: bool
    planes: List[str]
    duty: str


class AgentBus:
    """Formal attachment of the four agents to CactusOS."""

    def __init__(self):
        self.registry = AgentRegistry()
        self.engine = SI1Engine()

    def bind_all(self) -> List[AgentLink]:
        links: List[AgentLink] = []
        for agent_id, spec in ROLE_SPEC.items():
            role = self.registry.get(agent_id)
            bound = role.id == agent_id and agent_id in SLOT_MAP
            links.append(
                AgentLink(
                    agent_id=agent_id,
                    name=spec["name"],
                    slot=spec["slot"],
                    bound=bound,
                    planes=list(spec["planes"]),
                    duty=spec["duty"],
                )
            )
        return links

    def status_report(self) -> Dict[str, Any]:
        links = self.bind_all()
        return {
            "timestamp": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            "system": "CactusOS / HyperOS / SVL 5.6.0-QuantumWheel",
            "operator": "Mateusz Mitek",
            "bus": "si_1_core.agent_bus",
            "shared_backend": "same LLM instance (Ollama cactus-ai / Gemini / Cactus SI) — roles are personas, not separate models",
            "agents": [
                {
                    "id": L.agent_id,
                    "name": L.name,
                    "slot_hex": f"0x{L.slot:02X}",
                    "bound": L.bound,
                    "duty": L.duty,
                    "planes": L.planes,
                }
                for L in links
            ],
            "all_bound": all(L.bound for L in links),
            "providers": self.engine.status()["providers"],
            "mission": "Podłącz SI_1, Astyria, Aurelius, Alex pod system CactusOS — DONE",
        }


def main() -> int:
    report = AgentBus().status_report()
    print(json.dumps(report, indent=2, ensure_ascii=False))
    return 0 if report["all_bound"] else 1


if __name__ == "__main__":
    raise SystemExit(main())
