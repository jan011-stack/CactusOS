# CactusOS

![CactusOS](assets/logo.svg)

Experimental operating system research project combining a **no_std Rust kernel library** (HyperOS) with a **Python web terminal** that routes to Gemini or local Ollama.

> **Status:** Research / experimental. Not a production OS.

## Features

- **HyperOS** — bare-metal optional Rust crate (`no_std`, no heap) with scheduler, chrono log, Ollama client stubs
- **Gemini primary** → **Ollama fallback** (recommended model: `llama3.2:3b` on 16 GB RAM)
- Explicit approval required for any command execution

## Quick Start

```bash
git clone https://github.com/marcinwietrzala-stack/CactusOS.git
cd CactusOS
cp .env.example .env
# Add your GEMINI_API_KEY to .env

# Optional: set up local Ollama fallback
bash scripts/ollama_16gb.sh

# Web terminal
python3 -m hyperos.web_server --port 4180
```

## Provider Configuration

| Priority | Provider | Requirement |
|----------|----------|-------------|
| 1 | Gemini | `GEMINI_API_KEY` |
| 2 | Ollama | Local Ollama + model |

See [.env.example](.env.example).

## Project Layout

```
CactusOS/
├── src/                 # HyperOS Rust (no_std)
├── hyperos/             # Python web terminal and helpers
├── web/                 # Web terminal frontend
├── scripts/             # ollama_16gb.sh, ollama_eco.sh
├── docs/                # Additional documentation
├── assets/              # Logo and graphics
├── Modelfile            # Ollama model definition
└── LICENSE
```

## Documentation

- [Overview](docs/OVERVIEW.md)
- Ollama 16 GB setup: `scripts/ollama_16gb.sh`

## Safety Notes

- Commands require explicit user approval.
- Root approvals work on localhost and the configured Codespaces forwarded URL.
  Keep the forwarded port private: approved commands run with the server's root access.
  Other hosts/origins and requests without the session CSRF token are rejected.
- No automatic root or background privileged processes.
- Do not commit API keys. Use `.env` (gitignored).
- The fictional ship "Mateusz_1" and similar lore have no real telemetry.

## License

[MIT](LICENSE) © 2026 Mateusz Mitek

---

*Built for experimentation. Contributions welcome.*
