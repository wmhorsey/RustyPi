#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import json
from dataclasses import dataclass
from pathlib import Path


@dataclass
class Row:
    tick: int
    node: int
    energy: int
    coherence: int
    shell: int
    spin: int
    phase: str
    drive: int


def load_rows(path: Path) -> list[Row]:
    rows: list[Row] = []
    with path.open("r", encoding="utf-8", newline="") as f:
        r = csv.DictReader(f)
        for rec in r:
            rows.append(
                Row(
                    tick=int(rec["tick"]),
                    node=int(rec["node"]),
                    energy=int(rec["energy"]),
                    coherence=int(rec["coherence"]),
                    shell=int(rec["shell_ring_ticks"]),
                    spin=int(rec["spin_bias"]),
                    phase=(rec["phase"] or "").strip().lower(),
                    drive=int(rec["drive"]),
                )
            )
    return rows


def mean(vals: list[float]) -> float:
    return (sum(vals) / len(vals)) if vals else 0.0


def classify(metrics: dict[str, float], core_leak: int) -> str:
    if core_leak > 0:
        return "core-leak"
    if metrics["shell_to_far_tension_ratio"] > 1.0 and metrics["boundary_depletion_share"] > 0.2:
        return "void-shell-locked"
    if metrics["active_phase_share"] > 0.6:
        return "high-activity-flow"
    if metrics["abs_field_growth_ratio"] > 2.0:
        return "compressive-ramp"
    return "diffuse-free"


def analyze(rows: list[Row], center: int, radius: int) -> dict:
    core_lo = center - radius
    core_hi = center + radius

    shell_nodes = {
        core_lo - 2,
        core_lo - 1,
        core_lo,
        core_lo + 1,
        core_lo + 2,
        core_hi - 2,
        core_hi - 1,
        core_hi,
        core_hi + 1,
        core_hi + 2,
    }
    far_nodes = {0, 1, 2, 64, 128, 256}

    # Add mirrored far nodes if domain is large enough.
    max_node = max((r.node for r in rows), default=0)
    far_nodes |= {n for n in {max_node, max_node - 1, max_node - 2, max_node - 64, max_node - 128, max_node - 256} if n >= 0}

    first_tick = min(r.tick for r in rows)
    last_tick = max(r.tick for r in rows)

    abs_field_first: list[float] = []
    abs_field_last: list[float] = []

    shell_tension_peak = 0
    far_tension_peak = 0

    boundary_rows = 0
    boundary_depleted = 0

    core_nonzero = 0
    active_rows = 0

    shell_density_samples: list[float] = []
    far_density_samples: list[float] = []

    for r in rows:
        tension = min(r.energy, r.coherence)
        abs_field = abs(r.energy) + abs(r.coherence)

        if r.tick == first_tick:
            abs_field_first.append(abs_field)
        if r.tick == last_tick:
            abs_field_last.append(abs_field)

        if r.phase != "free":
            active_rows += 1

        if core_lo <= r.node <= core_hi and (r.energy != 0 or r.coherence != 0 or r.shell != 0 or r.spin != 0 or r.drive != 0):
            core_nonzero += 1

        if r.node in shell_nodes:
            shell_tension_peak = max(shell_tension_peak, tension)
            shell_density_samples.append(abs_field)
            boundary_rows += 1
            if min(r.energy, r.coherence) <= 0:
                boundary_depleted += 1

        if r.node in far_nodes:
            far_tension_peak = max(far_tension_peak, tension)
            far_density_samples.append(abs_field)

    shell_mean_density = mean(shell_density_samples)
    far_mean_density = mean(far_density_samples)

    metrics = {
        "shell_to_far_tension_ratio": shell_tension_peak / far_tension_peak if far_tension_peak > 0 else float("inf"),
        "boundary_contrast_number": abs(shell_mean_density - far_mean_density) / (far_mean_density + 1.0),
        "boundary_depletion_share": boundary_depleted / boundary_rows if boundary_rows else 0.0,
        "active_phase_share": active_rows / len(rows) if rows else 0.0,
        "abs_field_growth_ratio": mean(abs_field_last) / (mean(abs_field_first) + 1e-9),
    }

    return {
        "first_tick": first_tick,
        "last_tick": last_tick,
        "rows": len(rows),
        "core_nonzero_samples": core_nonzero,
        "metrics": metrics,
        "regime": classify(metrics, core_nonzero),
    }


def main() -> int:
    ap = argparse.ArgumentParser(description="Compute dimensionless regime-map metrics from decoded choke CSV")
    ap.add_argument("--csv", required=True, help="Decoded CSV path")
    ap.add_argument("--center", type=int, required=True, help="Core center node index")
    ap.add_argument("--radius", type=int, required=True, help="Core radius in nodes")
    ap.add_argument("--out", default=None, help="Optional JSON output path")
    args = ap.parse_args()

    csv_path = Path(args.csv)
    rows = load_rows(csv_path)
    result = analyze(rows, center=args.center, radius=args.radius)

    print(json.dumps(result, indent=2))

    if args.out:
        out = Path(args.out)
        out.write_text(json.dumps(result, indent=2), encoding="utf-8")
        print(f"wrote={out}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
