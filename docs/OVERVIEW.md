# CactusOS — Overview

CactusOS is an experimental project combining:

- **HyperOS** — a `no_std` Rust kernel-style library (bare-metal optional)
- **Web terminal** — Gemini / Ollama chat with approval-gated commands

## Current Provider Order

1. **Gemini** (primary) — requires `GEMINI_API_KEY`
2. **Ollama** (fallback) — local, default text model `llama3.1:8b`; use the eco profile on 16 GB hosts

The system uses a single **Cactus AI** role.

## Quick Start

```bash
git clone https://github.com/marcinwietrzala-stack/CactusOS.git
cd CactusOS
cp .env.example .env
# set GEMINI_API_KEY in .env

bash scripts/ollama_16gb.sh   # optional local fallback
python3 -m hyperos.web_server --port 4180
```

## Architecture

```
User → Web terminal → provider routing
                              ├─ Gemini (primary)
                              └─ Ollama (fallback)
         ↓
      Response + optional tool proposals (require explicit approval)
```

## What This Is Not

- Not a production operating system
- Not a trained language model (uses external Gemini or local Ollama)
- No automatic root access or background processes with elevated privileges
- No real telemetry for fictional elements (e.g. Mateusz_1)

## License

MIT — see [LICENSE](../LICENSE).
