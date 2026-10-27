"""Quick status CLI combining SVL kernel + matrix tile demo."""

from __future__ import annotations

import json
from .svl_kernel import SVLSystemKernel
from .matrix_tiles import demo_report


def main() -> int:
    kernel = SVLSystemKernel()
    status = kernel.status_report()
    tiles = json.loads(demo_report())
    combined = {
        "svl": status,
        "matrix_tiles": tiles,
        "note": "CactusOS Ollama-first, compact infinity, tile streaming for low RAM/CPU",
    }
    print(json.dumps(combined, indent=2, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
