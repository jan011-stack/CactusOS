#!/usr/bin/env python3
"""CLI for SI_1 engine.

Usage:
  python3 -m si_1_core.cli "Twoje pytanie"
  python3 -m si_1_core.cli --mission "Przeanalizuj architekturę HyperOS"
  python3 -m si_1_core.cli --status
"""

from __future__ import annotations

import argparse
import json
import sys

from .engine import SI1Engine


def main(argv: list[str] | None = None) -> int:
    p = argparse.ArgumentParser(description="SI_1 NLP/LLM core for CactusOS")
    p.add_argument("prompt", nargs="?", help="User message")
    p.add_argument("--agent", default="si_1", help="Agent id: si_1|alex|astyria|aurelius")
    p.add_argument("--mission", action="store_true", help="Run multi-agent mission")
    p.add_argument("--status", action="store_true", help="Print engine status")
    p.add_argument("--provider", choices=["ollama", "gemini", "cactus_si"], default=None)
    p.add_argument("--json", action="store_true", help="JSON output")
    args = p.parse_args(argv)

    engine = SI1Engine()

    if args.status:
        print(json.dumps(engine.status(), indent=2, ensure_ascii=False))
        return 0

    if not args.prompt:
        p.print_help()
        return 1

    if args.mission:
        results = engine.mission(args.prompt)
        if args.json:
            print(json.dumps({k: v.to_dict() for k, v in results.items()}, indent=2, ensure_ascii=False))
        else:
            for aid, resp in results.items():
                print(f"\n=== {resp.agent} ({resp.provider}/{resp.model}) ===")
                print(resp.text)
        return 0

    resp = engine.chat(args.prompt, agent_id=args.agent, prefer_provider=args.provider)
    if args.json:
        print(json.dumps(resp.to_dict(), indent=2, ensure_ascii=False))
    else:
        print(f"[{resp.agent} | {resp.provider}/{resp.model} | intent={resp.intent.value}]")
        print(resp.text)
        if resp.proposals:
            print("\n--- Tool proposals (require WYKONAJ) ---")
            for i, prop in enumerate(resp.proposals, 1):
                print(f"{i}. {prop.command}")
                if prop.rationale:
                    print(f"   reason: {prop.rationale}")
    return 0 if resp.ok else 2


if __name__ == "__main__":
    sys.exit(main())
