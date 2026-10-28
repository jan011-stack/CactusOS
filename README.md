# CactusOS

![CactusOS](assets/logo.svg)

Experimental operating system research project combining a **no_std Rust kernel library** (HyperOS) with a **Python web terminal** that routes to Gemini or local Ollama.

> **Status:** Research / experimental. Not a production OS.

## Features

- **HyperOS** — bare-metal optional Rust crate (`no_std`, no heap) with scheduler, chrono log, Ollama client stubs
- **Gemini primary** → **Ollama fallback** (default local text model: `llama3.1:8b`)
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
- In the web terminal, enter `approve` (or `accept` / `yes`) to approve the single pending terminal command; `reject` (or `deny` / `no`) rejects it. Proposals expire after 10 minutes.
- The root terminal can run a selected workspace Python script only after showing its full source and receiving explicit approval. Scripts are limited to 64 KiB, 60 seconds, and 64 KiB of captured output; administrator access must be available.
- Root approvals work on localhost and the configured Codespaces forwarded URL.
  Keep the forwarded port private: approved commands run with the server's root access.
  Other hosts/origins and requests without the session CSRF token are rejected.
- The web server does not shut down when idle. GitHub Codespaces can stop an inactive container according to its configured idle timeout; adjust that timeout in Codespaces settings if needed, then restart the server with `python3 -m hyperos.web_server --port 4180`.
- No automatic root or background privileged processes.
- The terminal keeps up to 40 text exchanges in this browser's local storage; attached image contents are not persisted. Explorer download responses are recorded locally by the server in `${XDG_STATE_HOME:-~/.local/state}/cactusos/downloads.jsonl` and can be viewed from the Explorer. A `response_written` entry confirms the server wrote the response, not that the browser saved it. The chat model cannot make purchases or download internet files.
- Do not commit API keys. Use `.env` (gitignored).
- The fictional ship "Mateusz_1" and similar lore have no real telemetry.

## License

[MIT](LICENSE) © 2026 Mateusz Mitek

---

*Built for experimentation. Contributions welcome.*
