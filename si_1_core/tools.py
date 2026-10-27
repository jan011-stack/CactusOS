"""Safe tool proposal protocol — never auto-executes.

Matches CactusOS policy: show full command, require explicit WYKONAJ.
"""

from __future__ import annotations

import json
import re
import subprocess
from dataclasses import dataclass
from typing import Any, Dict, List, Optional


@dataclass
class ToolProposal:
    command: str
    rationale: str
    cwd: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {"command": self.command, "rationale": self.rationale, "cwd": self.cwd}


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
        if isinstance(obj, dict) and "command" in obj:
            out.append(
                ToolProposal(
                    command=str(obj["command"]),
                    rationale=str(obj.get("rationale", "")),
                    cwd=obj.get("cwd"),
                )
            )
        elif isinstance(obj, list):
            for item in obj:
                if isinstance(item, dict) and "command" in item:
                    out.append(
                        ToolProposal(
                            command=str(item["command"]),
                            rationale=str(item.get("rationale", "")),
                            cwd=item.get("cwd"),
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
                "hint": "Pass approve=True only after operator types WYKONAJ",
                "command": proposal.command,
            }
        cwd = proposal.cwd or self.default_cwd
        try:
            proc = subprocess.run(
                proposal.command,
                shell=True,
                cwd=cwd,
                capture_output=True,
                text=True,
                timeout=self.timeout,
            )
            stdout = (proc.stdout or "")[:65536]
            stderr = (proc.stderr or "")[:65536]
            return {
                "ok": proc.returncode == 0,
                "returncode": proc.returncode,
                "stdout": stdout,
                "stderr": stderr,
                "command": proposal.command,
            }
        except subprocess.TimeoutExpired:
            return {"ok": False, "error": "timeout", "command": proposal.command}
        except Exception as e:
            return {"ok": False, "error": str(e), "command": proposal.command}
