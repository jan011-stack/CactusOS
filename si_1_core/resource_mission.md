# Misja si_1 — mniejsze RAM/CPU + usprawniona Ollama

**Status:** WYKONANA · 2026-10-27  
**Operator:** Mateusz Mitek

## Zmiany

### 1. Profile zasobów (`hyperos/agents.py`)
| Profil | ctx | threads | predict | keep_alive | historia |
|--------|-----|---------|---------|------------|----------|
| **eco** (domyślny) | 512 | 1 | 96 | 0 (unload) | 8 |
| balanced | 1024 | 2 | 160 | 2m | 12 |
| quality | 2048 | 4 | 256 | 5m | 16 |

```bash
export CACTUS_OLLAMA_PROFILE=eco        # domyślnie
export CACTUS_OLLAMA_PROFILE=balanced
export CACTUS_OLLAMA_MODEL=cactus-ai-lite
```

### 2. Modele Ollama
- `Modelfile` — eco + top_k/top_p (mniej fluktuacji CPU)
- `Modelfile.lite` — **llama3.2:1b** text-only (dużo mniej RAM niż vision)
- `Modelfile.balanced` — średni kompromis

### 3. Runtime Ollama
```bash
export OLLAMA_NUM_PARALLEL=1
export OLLAMA_MAX_LOADED_MODELS=1
export OLLAMA_KEEP_ALIVE=0
export OLLAMA_FLASH_ATTENTION=1
./scripts/ollama_eco.sh          # default eco
./scripts/ollama_eco.sh lite     # najlżejszy model
./scripts/ollama_eco.sh balanced
```

### 4. Boot
`hyper_boot.sh` ustawia limity eco, opcjonalnie:
```bash
HYPEROS_OLLAMA_LITE=1 HYPEROS_SKIP_WARMUP=1 ./hyper_boot.sh
```

## Efekt oczekiwany
- 1 model naraz, 1 request równolegle
- Wagi zrzucane z RAM po odpowiedzi (eco)
- Lite ≈ rząd wielkości mniej pamięci niż llama3.2-vision
- Agent roles nadal współdzielą jedną instancję (bez 4× load)

## Co nadal jest ciężkie
- `llama3.2-vision` sam w sobie — do czatu tekstowego używaj `cactus-ai-lite`
- Pierwsze ładowanie modelu zawsze kosztuje RAM; `keep_alive=0` zwalnia po requestcie
