"""SI_1 Core — NLP + Ollama engine for CactusOS / HyperOS.

Ollama primary, local NLP fallback, matrix tiles for low RAM/CPU,
SVL compact infinity kernel.
"""

from .engine import SI1Engine, SI1Config
from .nlp import NLPPipeline, Intent
from .providers import ProviderRouter
from .agents import AgentRegistry, AgentRole
from .memory import ConversationMemory
from .tools import ToolProposal, ToolExecutor
from .svl_kernel import SVLSystemKernel, TopologicalInfinityEngine

__version__ = "1.3.0-Ollama16GB"
__all__ = [
    "SI1Engine",
    "SI1Config",
    "NLPPipeline",
    "Intent",
    "ProviderRouter",
    "AgentRegistry",
    "AgentRole",
    "ConversationMemory",
    "ToolProposal",
    "ToolExecutor",
    "SVLSystemKernel",
    "TopologicalInfinityEngine",
]
