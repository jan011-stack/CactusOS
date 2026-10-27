# CactusOS / si_1 — aktualizacja 2026 (internet + GitHub)

**Data:** 2026-10-27  
**Operator:** Mateusz Mitek  
**Źródła:** OSDev / rust-osdev, Ollama docs & 2026 guides, Remco123/CactusOS (upstream), GitHub Actions patterns.

## Co zaktualizowano

### 1. Lokalny NLP (bez API / Ollamy)
- Nowy `local_nlp.py` — odpowiedzi szablonowe z intencji NLP.
- `ProviderRouter` automatycznie spada do local NLP, gdy brak backendu lub `CACTUS_LOCAL_ONLY=1`.
- Engine przekazuje wynik NLP do routera.

**Uruchomienie offline:**
```bash
export CACTUS_LOCAL_ONLY=1
python3 -m si_1_core "jaki jest status systemu?"
```

### 2. Ollama eco — rekomendacje 2026
Z aktualnych źródeł (ComputingForGeeks, selfhostmap, vc.ru):
- CPU-only: preferuj `llama3.2:1b` / `qwen3.5:2b` (ok. 1–2 GB RAM przy krótkim kontekście).
- `OLLAMA_KEEP_ALIVE=0` + `keep_alive: 0` w requestcie — zrzuca wagi po odpowiedzi.
- `OLLAMA_MAX_LOADED_MODELS=1`, `OLLAMA_NUM_PARALLEL=1`.
- Opcjonalnie `OLLAMA_KV_CACHE_TYPE=q8_0` (mniejszy cache, prawie bez utraty jakości).
- Vision (`llama3.2-vision`) jest ciężki — do czatu tekstowego użyj lite.

### 3. Hobby OS / HyperOS
- Upstream Remco123/CactusOS nadal z 2021 (V1.2). Twoje repo jest kontynuacją z HyperOS + si_1.
- Rust `no_std` + QEMU + GitHub Actions to aktualny standard (rust-osdev, xv6-rust, rustrial-os).
- Dodano workflow CI: Python tests + `cargo check`.

### 4. Matrix tiles
- Nadal aktywne: kafelkowanie zamiast pełnej macierzy → peak RAM = rozmiar kafelka.

## Jak używać

```bash
# Pełny status
python3 -m si_1_core --status

# Tylko lokalne NLP
CACTUS_LOCAL_ONLY=1 python3 -m si_1_core "analizuj system"

# Eco Ollama
./scripts/ollama_eco.sh lite
export CACTUS_OLLAMA_PROFILE=eco
export CACTUS_OLLAMA_MODEL=cactus-ai-lite
```

## Następne kroki (rekomendowane)

1. Uruchomić workflow CI na push.
2. Przetestować `CACTUS_LOCAL_ONLY=1`.
3. Dla prawdziwego generowania: `ollama create cactus-ai-lite -f Modelfile.lite`.
4. Rozważyć migrację HyperOS w stronę 64-bit UEFI (jak współczesne hobby OS), jeśli planujesz dalszy rozwój kernela.

**Status misji:** WYKONANA — system zaktualizowany maksymalnie na bazie dostępnych danych z internetu i podłączonego repozytorium GitHub.
