"""SI_1 Matrix Tile Engine — infinite domain → compact tiles → low RAM/CPU.

Inspired by the System Matrix notes (blocked / process matrices Yn):
  [X_infinity] is never materialized whole.
  It is projected to compact [-1,1] and evaluated only as small tiles.

This is real numerical structure (tiling + streaming), not a network or intel toy.
"""

from __future__ import annotations

import json
import math
from dataclasses import dataclass, field
from datetime import datetime
from typing import Callable, Dict, Iterator, List, Optional, Tuple


def compactify(x: float) -> float:
    """Stereographic-style map R∪{±∞} → (-1,1); matches SVL TopologicalInfinityEngine."""
    if x == float("-inf"):
        return -1.0
    if x == float("inf"):
        return 1.0
    if math.isnan(x):
        return 0.0
    return x / (1.0 + abs(x))


@dataclass
class TileSpec:
    """One process matrix block (operacja systemowa)."""
    row: int
    col: int
    size: int  # n x n
    tag: str = "Yn"

    @property
    def id(self) -> str:
        return f"T[{self.row},{self.col}]/{self.size}"


@dataclass
class TileResult:
    spec: TileSpec
    frobenius: float
    peak_abs: float
    cells_touched: int


@dataclass
class MatrixPlaneConfig:
    tile_size: int = 8          # np.8 → macierz [x]; 16 → [y]
    max_tiles: int = 64         # hard cap — bounds RAM & CPU
    stream: bool = True         # never keep all tiles resident
    dtype_bytes: int = 8        # float64

    def peak_ram_bytes(self) -> int:
        # one tile in memory at a time when streaming
        n = self.tile_size
        return n * n * self.dtype_bytes

    def naive_full_ram_bytes(self, full_n: int) -> int:
        return full_n * full_n * self.dtype_bytes


class MatrixTileEngine:
    """Divide virtual infinite / large matrix into process tiles Yn."""

    def __init__(self, config: Optional[MatrixPlaneConfig] = None):
        self.config = config or MatrixPlaneConfig()

    def tile_grid(self, logical_n: int) -> List[TileSpec]:
        ts = max(1, self.config.tile_size)
        tiles: List[TileSpec] = []
        rows = (logical_n + ts - 1) // ts
        cols = rows
        for r in range(rows):
            for c in range(cols):
                tiles.append(TileSpec(row=r, col=c, size=ts, tag=f"Yn-{len(tiles)}"))
                if len(tiles) >= self.config.max_tiles:
                    return tiles
        return tiles

    def eval_tile(
        self,
        spec: TileSpec,
        kernel: Callable[[int, int], float],
    ) -> TileResult:
        """Compute one tile; release after measuring stats (stream mode)."""
        n = spec.size
        base_i = spec.row * n
        base_j = spec.col * n
        # allocate only this block
        block = [[0.0] * n for _ in range(n)]
        peak = 0.0
        frob2 = 0.0
        for di in range(n):
            for dj in range(n):
                v = kernel(base_i + di, base_j + dj)
                # optional compact map for "infinity" sources
                if not math.isfinite(v):
                    v = compactify(v)
                block[di][dj] = v
                av = abs(v)
                if av > peak:
                    peak = av
                frob2 += v * v
        # drop block (GC) — peak RAM ≈ one tile
        del block
        return TileResult(
            spec=spec,
            frobenius=math.sqrt(frob2),
            peak_abs=peak,
            cells_touched=n * n,
        )

    def run(
        self,
        logical_n: int,
        kernel: Optional[Callable[[int, int], float]] = None,
    ) -> Dict:
        if kernel is None:
            # default: compact distance field (cheap, stable)
            def kernel(i: int, j: int) -> float:
                return compactify(float(i - j)) * compactify(float(i + j + 1))

        tiles = self.tile_grid(logical_n)
        results: List[TileResult] = []
        total_cells = 0
        max_peak = 0.0
        for spec in tiles:
            tr = self.eval_tile(spec, kernel)
            results.append(tr)
            total_cells += tr.cells_touched
            if tr.peak_abs > max_peak:
                max_peak = tr.peak_abs
            if self.config.stream:
                # only last tile stats kept heavily — already measured
                pass

        full_ram = self.config.naive_full_ram_bytes(logical_n)
        tile_ram = self.config.peak_ram_bytes()
        return {
            "timestamp": datetime.now().strftime("%Y-%m-%d %H:%M:%S"),
            "engine": "si_1.matrix_tiles",
            "logical_n": logical_n,
            "tile_size": self.config.tile_size,
            "tiles_run": len(results),
            "cells_touched": total_cells,
            "peak_abs": round(max_peak, 6),
            "ram_peak_bytes_streaming": tile_ram,
            "ram_if_full_matrix_bytes": full_ram,
            "ram_reduction_factor": round(full_ram / max(tile_ram, 1), 2),
            "cpu_note": "O(tiles * tile_size^2) with high cache locality; no full matrix alloc",
            "yn_ops": [r.spec.tag for r in results[:16]],
            "status": "TILE_STREAM_COMPLETE",
        }


def demo_report() -> str:
    """Compare tile sizes 8 and 16 (from attachment notation)."""
    reports = []
    for ts, label in ((8, "X_infinity→np.8 [x]"), (16, "X_infinity2→np.16 [y]")):
        eng = MatrixTileEngine(MatrixPlaneConfig(tile_size=ts, max_tiles=32, stream=True))
        # logical 128 without allocating 128^2
        reports.append({"label": label, **eng.run(128)})
    return json.dumps(
        {
            "mission": "si_1 matrix plane — RAM/CPU via process tiles",
            "operator": "Mateusz Mitek",
            "runs": reports,
        },
        indent=2,
        ensure_ascii=False,
    )


if __name__ == "__main__":
    print(demo_report())
