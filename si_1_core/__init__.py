"""SI_1 Core — NLP + LLM engine for CactusOS / HyperOS.

Multi-provider routing (Ollama → Gemini → Cactus SI),
intent NLP, role agents, bounded memory, and safe tool proposals.
"""

from .engine import SI1Engine, SI1Config
from .nlp import NLPPipeline, Intent
from .providers import ProviderRouter
from .agents import AgentRegistry, AgentRole
from .memory import ConversationMemory
from .tools import ToolProposal, ToolExecutor

__version__ = "1.0.0-QuantumWheel"
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
]
