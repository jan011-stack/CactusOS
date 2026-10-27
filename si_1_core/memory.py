"""Bounded conversation memory — no unbounded growth."""

from __future__ import annotations

from collections import deque
from dataclasses import dataclass, field
from typing import Deque, List, Optional

from .providers import ChatMessage


@dataclass
class ConversationMemory:
    max_turns: int = 24  # user+assistant pairs roughly
    _messages: Deque[ChatMessage] = field(default_factory=deque)

    def add(self, role: str, content: str) -> None:
        self._messages.append(ChatMessage(role=role, content=content))
        while len(self._messages) > self.max_turns * 2:
            self._messages.popleft()

    def as_list(self, system: Optional[str] = None) -> List[ChatMessage]:
        out: List[ChatMessage] = []
        if system:
            out.append(ChatMessage(role="system", content=system))
        out.extend(self._messages)
        return out

    def clear(self) -> None:
        self._messages.clear()

    def __len__(self) -> int:
        return len(self._messages)
