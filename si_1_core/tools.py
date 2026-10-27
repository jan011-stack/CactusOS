"""Safe tool proposal protocol — never auto-executes.

Matches CactusOS policy: show full command, require explicit WYKONAJ.
Supports optional elevated (root via sudo -n) after approve=True.
"""

from __future__ import annotations

import json
import re
from dataclasses import dataclass
from typing import Any, Dict, List, Optional

from .terminal_fix import execute_command as _exec, root_available


@dataclass
class ToolProposal:
    command: str
    rationale: str
    cwd: Optional[str] = None
    elevated: bool = False  # hyperspace root path

    def to_dict(self) -> Dict[str, Any]:
        return {
            "command": self.command,
            "rationale": self.rationale,
            "cwd": self.cwd,
            "elevated": self.elevated,
        }


_PROPOSAL_RE = re.compile(
    r"```json\s*(\{.*?\})\s*```",
    re.S | re.I,
)


def parse_tool_proposals(llm_text: str) -> List[ToolProposal]:
    """Extract JSON tool proposals from model output."""
    out: List[ToolProposal] = []
    for m in _PROPOSAL_RE.finditer(llm_text or ""):
        try:
            obj = json.loads(m.group(1))
        except json.JSONDecodeError:
            continue
        items = obj if isinstance(obj, list) else [obj] if isinstance(obj, dict) else []
        for item in items:
            if isinstance(item, dict) and "command" in item:
                out.append(
                    ToolProposal(
                        command=str(item["command"]),
                        rationale=str(item.get("rationale", "")),
                        cwd=item.get("cwd"),
                        elevated=bool(item.get("elevated", False)),
                    )
                )
    return out


class ToolExecutor:
    """Runs approved commands only — operator must pass approve=True."""

    def __init__(self, default_cwd: Optional[str] = None, timeout: int = 60):
        self.default_cwd = default_cwd
        self.timeout = timeout

    def run(self, proposal: ToolProposal, approve: bool = False) -> Dict[str, Any]:
        if not approve:
            return {
                "ok": False,
                "error": "not_approved",
                "hint": "Pass approve=True only after operator types WYKONAJ / checks consent",
                "command": proposal.command,
                "elevated": proposal.elevated,
                "root_available": root_available(),
            }
        result = _exec(
            proposal.command,
            elevated=proposal.elevated,
            approve=True,
            timeout_sec=self.timeout,
        )
        return result.to_dict()
