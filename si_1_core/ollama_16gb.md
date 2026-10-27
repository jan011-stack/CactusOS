# Ollama dla 16 GB RAM (Codespaces)

**Cactus SI usunięte.** Główny backend: lokalna Ollama.

## Zalecany model
- `llama3.2:3b` — najlepszy kompromis jakość/RAM/szybkość (~2 GB)
- Unikaj `llama3.2-vision` (7.8 GB+) — za ciężki dla Codespaces

## Uruchomienie
```bash
bash scripts/ollama_16gb.sh
```

## Zmienne
```env
CACTUS_OLLAMA_MODEL=llama3.2:3b
CACTUS_OLLAMA_PROFILE=balanced
OLLAMA_MAX_LOADED_MODELS=1
OLLAMA_FLASH_ATTENTION=1
```

## Terminal większy + pasek postępu (jak w Copilot)
W GitHub Codespaces:
1. Otwórz panel Terminal (Ctrl+`)
2. Przeciągnij separator w górę — terminal staje się większy
3. Przy `ollama pull` pasek postępu pojawia się automatycznie
4. Skrypty z `echo` i `set -x` pokazują wykonywane komendy

W panelu webowym CactusOS terminal jest wbudowany; większy widok uzyskasz przez pełny ekran przeglądarki lub layout Codespaces.
