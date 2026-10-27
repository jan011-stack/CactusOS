#!/usr/bin/env python3
"""SI_1 terminal / hyperspace root path fixer and diagnostics.

Policy (unchanged by design):
- User terminal: execute as current user after explicit approve
- Hyperspace root terminal: execute via `sudo -n` after explicit approve
- Never auto-run as root without operator confirmation
- Never strip WYKONAJ / consent gates

This module makes approved commands *work* when the environment is configured.
"""

from __future__ import annotations

import json
import os
import selectors
import subprocess
import sys
from dataclasses import dataclass
from pathlib import Path
from typing import Any, Dict, Optional

WORKSPACE = Path(__file__).resolve().parents[1]


@dataclass
class ExecResult:
    ok: bool
    exit_code: int
    output: str
    truncated: bool
    elevated: bool
    error: Optional[str] = None

    def to_dict(self) -> Dict[str, Any]:
        return {
            "ok": self.ok,
            "exit_code": self.exit_code,
            "output": self.output,
            "truncated": self.truncated,
            "elevated": self.elevated,
            "error": self.error,
        }


def root_available() -> bool:
    """True when passwordless sudo can obtain uid 0 (sudo -n)."""
    try:
        result = subprocess.run(
            ["sudo", "-n", "--", "/usr/bin/id", "-u"],
            capture_output=True,
            timeout=5,
            check=False,
        )
        return result.returncode == 0 and result.stdout.strip() == b"0"
    except (OSError, subprocess.SubprocessError):
        return False


def diagnose() -> Dict[str, Any]:
    """Report why terminal / root hyperspace may fail."""
    sudo_path = None
    for candidate in ("/usr/bin/sudo", "/bin/sudo"):
        if Path(candidate).is_file():
            sudo_path = candidate
            break
    timeout_path = "/usr/bin/timeout" if Path("/usr/bin/timeout").is_file() else None
    bash_path = "/bin/bash" if Path("/bin/bash").is_file() else None
    available = root_available()
    hints = []
    if not sudo_path:
        hints.append("Brak sudo — zainstaluj sudo.")
    elif not available:
        hints.append(
            "sudo -n nie daje uid 0. Skonfiguruj NOPASSWD dla operatora "
            "(patrz si_1_core/TERMINAL_ROOT.md) albo uruchom lokalnie, nie zdalnie."
        )
    if not timeout_path:
        hints.append("Brak /usr/bin/timeout — required by execute_command.")
    if not bash_path:
        hints.append("Brak /bin/bash.")
    if os.geteuid() == 0:
        hints.append(
            "Proces działa już jako root — web_server odmawia trybu użytkownika; "
            "hiperterminal root wymaga lokalnego Host + CSRF."
        )
    return {
        "workspace": str(WORKSPACE),
        "euid": os.geteuid(),
        "sudo_path": sudo_path,
        "timeout_path": timeout_path,
        "bash_path": bash_path,
        "root_available": available,
        "hints": hints,
        "policy": {
            "user_terminal": "execute as current user after approve",
            "hyperspace_root": "sudo -n after explicit operator consent",
            "auto_root": False,
        },
    }


def execute_command(command: str, *, elevated: bool = False, approve: bool = False,
                    timeout_sec: int = 60, max_output: int = 65536) -> ExecResult:
    """Run command only when approve=True. elevated uses sudo -n."""
    if not approve:
        return ExecResult(
            ok=False, exit_code=-1, output="", truncated=False, elevated=elevated,
            error="not_approved — operator must confirm (WYKONAJ / consent checkbox)",
        )
    if not command or not command.strip():
        return ExecResult(ok=False, exit_code=-1, output="", truncated=False,
                          elevated=elevated, error="empty_command")
    if elevated and not root_available():
        return ExecResult(
            ok=False, exit_code=-1, output="", truncated=False, elevated=True,
            error="root_unavailable — sudo -n cannot obtain uid 0",
        )

    prefix = ["sudo", "-n", "--"] if elevated else []
    timeout_bin = "/usr/bin/timeout" if Path("/usr/bin/timeout").is_file() else None
    bash = "/bin/bash" if Path("/bin/bash").is_file() else "bash"

    if timeout_bin:
        argv = [*prefix, timeout_bin, "-k", "5s", f"{timeout_sec}s", bash, "-c", command]
    else:
        argv = [*prefix, bash, "-c", command]

    try:
        process = subprocess.Popen(
            argv,
            cwd=str(WORKSPACE),
            stdin=subprocess.DEVNULL,
            stdout=subprocess.PIPE,
            stderr=subprocess.STDOUT,
        )
        output = bytearray()
        total = 0
        truncated = False
        assert process.stdout is not None
        with selectors.DefaultSelector() as selector:
            selector.register(process.stdout, selectors.EVENT_READ)
            while selector.get_map():
                for key, _ in selector.select(timeout=1):
                    chunk = os.read(key.fileobj.fileno(), 4096)
                    if not chunk:
                        selector.unregister(key.fileobj)
                        break
                    total += len(chunk)
                    if total <= max_output:
                        output.extend(chunk)
                    else:
                        truncated = True
                if process.poll() is not None and not selector.get_map():
                    break
                if process.poll() is not None:
                    # drain remaining
                    while True:
                        chunk = os.read(process.stdout.fileno(), 4096)
                        if not chunk:
                            break
                        total += len(chunk)
                        if total <= max_output:
                            output.extend(chunk)
                        else:
                            truncated = True
                    break
        exit_code = process.wait(timeout=timeout_sec + 10)
        text = output.decode("utf-8", errors="replace")
        return ExecResult(
            ok=exit_code == 0,
            exit_code=exit_code,
            output=text,
            truncated=truncated,
            elevated=elevated,
        )
    except Exception as e:
        return ExecResult(
            ok=False, exit_code=-1, output="", truncated=False,
            elevated=elevated, error=str(e),
        )


def main(argv: list[str] | None = None) -> int:
    import argparse
    p = argparse.ArgumentParser(description="SI_1 terminal/root diagnostics and approved exec")
    p.add_argument("--diagnose", action="store_true")
    p.add_argument("--command", help="Command to run (requires --approve)")
    p.add_argument("--root", action="store_true", help="Elevated via sudo -n")
    p.add_argument("--approve", action="store_true", help="Explicit operator approval")
    args = p.parse_args(argv)

    if args.diagnose or not args.command:
        print(json.dumps(diagnose(), indent=2, ensure_ascii=False))
        if not args.command:
            return 0

    result = execute_command(args.command, elevated=args.root, approve=args.approve)
    print(json.dumps(result.to_dict(), indent=2, ensure_ascii=False))
    return 0 if result.ok else 1


if __name__ == "__main__":
    raise SystemExit(main())
