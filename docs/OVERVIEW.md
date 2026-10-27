# CactusOS — Overview

CactusOS is an experimental project combining:

- **HyperOS** — a `no_std` Rust kernel-style library (bare-metal optional)
- **Cognitive core** (`si_1_core`) — NLP intent layer + LLM provider router
- **Local tooling** — Ollama integration, file transfer, matrix tile compute

## Current Provider Order

1. **Gemini** (primary) — requires `GEMINI_API_KEY`
2. **Ollama** (fallback) — local, recommended model `llama3.2:3b` for 16 GB RAM

Local NLP fallback has been removed. Multi-role agents (Alex / Astyria / Aurelius / SI_1) are no longer the primary interface; the system uses a single **Cactus AI** role.

## Quick Start

```bash
git clone https://github.com/marcinwietrzala-stack/CactusOS.git
cd CactusOS
cp .env.example .env
# set GEMINI_API_KEY in .env

bash scripts/ollama_16gb.sh   # optional local fallback
python3 -m si_1_core.svl_kernel
```

## Architecture

```
User → NLP (intent/safety) → ProviderRouter
                              ├─ Gemini (primary)
                              └─ Ollama (fallback)
         ↓
      Response + optional tool proposals (require explicit approval)
```

## What This Is Not

- Not a production operating system
- Not a trained language model (uses external Gemini or local Ollama)
- No automatic root access or background agents with elevated privileges
- No real telemetry for fictional elements (e.g. Mateusz_1)

## License

MIT — see [LICENSE](../LICENSE).
