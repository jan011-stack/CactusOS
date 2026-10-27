# Misja si_1 — macierz systemowa → mniej RAM/CPU

Źródło koncepcji: notatki *System matrix* (nieskończona macierz dzielona na mniejsze macierze procesów Yn).

## Idea

```
[X_infinity]  ──compactify──►  (-1,1)
       │
       ▼
  kafelki np.8 / np.16   (TileSpec Yn)
       │
       ▼
  stream: 1 kafelek w RAM naraz  →  peak RAM = tile_size² × 8 B
```

**Całość obliczeń bez trzymania całości** — zgodne z hasłem „obliczenia bez pełnej materializacji”.

## Co wdrożono

| Element | Plik |
|---------|------|
| Silnik kafelkowy | `si_1_core/matrix_tiles.py` |
| Profile Ollama eco | `hyperos/agents.py`, `scripts/ollama_eco.sh` |
| keep_alive=0 | unload wag po requestcie |

## Uruchomienie

```bash
python3 -m si_1_core.matrix_tiles
```

Przykład: logical 128×128 przy tile 8 → peak ~512 B na kafelek zamiast ~128 KiB pełnej macierzy float64.

## Czego NIE wzięto z załączników Abarkus

Symulacje „przekierowania ruchu USA”, „sprzedaży technologii 2nm”, „baz CIA/KGB” **nie są** częścią CactusOS.
To narracyjne mocki — nie implementujemy ich jako funkcji systemu.

si_1 wdraża wyłącznie warstwę obliczeniową i zasobową.
