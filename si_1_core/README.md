# SI_1 Core — NLP + LLM for CactusOS

Najlepszy możliwy rdzeń SI w ramach repozytorium CactusOS (HyperOS / SVL).

## Funkcje

| Moduł | Rola |
|-------|------|
| `nlp.py` | Intent, język PL/EN, encje, sanitizacja prompt injection |
| `providers.py` | Router: Cactus SI → Gemini → Ollama (lokalnie) |
| `agents.py` | SI_1, Alex, Astyria, Aurelius |
| `memory.py` | Ograniczona pamięć rozmowy (per agent) |
| `tools.py` | Propozycje komend JSON + wykonanie tylko po `approve=True` |
| `engine.py` | Orkiestracja |
| `cli.py` | Interfejs wiersza poleceń |

## Uruchomienie

```bash
# Status
python3 -m si_1_core --status

# Czat
python3 -m si_1_core "Jaki jest stan HyperOS?"

# Misja multi-agent
python3 -m si_1_core --mission "Przeanalizuj ryzyko w hyper_ollama_client"

# JSON
python3 -m si_1_core --json "Napisz funkcję w Rust bez alokacji"
```

## Konfiguracja (`.env`)

```env
GEMINI_API_KEY=
CACTUS_SI_API_URL=https://cactus-si.ai
CACTUS_SI_API_KEY=
```

Bez kluczy działa lokalne Ollama (`cactus-ai:latest` na `127.0.0.1:11434`).

## Bezpieczeństwo

- Nie wykonuje poleceń automatycznie
- Wykrywa proste wzorce prompt-injection
- Pamięć ograniczona (domyślnie 24 tury)
- Limit wyjścia / timeout po stronie providera i tool executora

## Integracja z HyperOS

Warstwa Python współpracuje z istniejącymi modułami Rust:
`hyper_ollama_client`, `hyper_agent_bridge`, `hyper_chat_stream`, `cognitive_bridge`.
Nie zastępuje `no_std` kernela — to host-side intelligence plane.

Operator: **Mateusz Mitek** · SVL 5.6.0-QuantumWheel · Agent: **si_1**
