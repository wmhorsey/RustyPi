#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
from dataclasses import dataclass
from pathlib import Path


@dataclass(frozen=True)
class Row:
    tick: int
    node: int
    coherence: float
    energy: float
    phase: str


def normalize_phase(s: str) -> str:
    s = (s or "").strip().lower()
    aliases = {
        "liftoff": "liftoff",
        "coherence": "coherence",
        "formation": "formation",
        "drift": "drift",
        "dissolution": "dissolution",
        "free": "free",
    }
    return aliases.get(s, s)


def phase_bucket(phase: str) -> str:
    if phase == "free":
        return "free"
    return "active"


def load_rows(path: Path) -> dict[tuple[int, int], Row]:
    out: dict[tuple[int, int], Row] = {}
    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        required = {"tick", "node", "coherence", "energy", "phase"}
        missing = required - set(reader.fieldnames or [])
        if missing:
            raise ValueError(f"{path}: missing columns {sorted(missing)}")

        for rec in reader:
            tick = int(rec["tick"])
            node = int(rec["node"])
            row = Row(
                tick=tick,
                node=node,
                coherence=float(rec["coherence"]),
                energy=float(rec["energy"]),
                phase=normalize_phase(rec["phase"]),
            )
            out[(tick, node)] = row
    return out


def mean_abs_err(values: list[float]) -> float:
    if not values:
        return 0.0
    return sum(values) / len(values)


def main() -> int:
    ap = argparse.ArgumentParser(description="Compare legacy and RustyPi choke traces")
    ap.add_argument("--legacy", required=True, help="Legacy CSV path")
    ap.add_argument("--rustypi", required=True, help="RustyPi CSV path")
    args = ap.parse_args()

    legacy = load_rows(Path(args.legacy))
    rusty = load_rows(Path(args.rustypi))

    keys = sorted(set(legacy.keys()) & set(rusty.keys()))
    if not keys:
        print("No overlapping (tick,node) rows between traces.")
        return 1

    coh_err: list[float] = []
    en_err: list[float] = []
    phase_match = 0
    bucket_match = 0

    phase_counts_legacy: dict[str, int] = {}
    phase_counts_rusty: dict[str, int] = {}
    transition_counts: dict[tuple[str, str], int] = {}
    legacy_phases: set[str] = set()
    rusty_phases: set[str] = set()

    for k in keys:
        a = legacy[k]
        b = rusty[k]
        coh_err.append(abs(a.coherence - b.coherence))
        en_err.append(abs(a.energy - b.energy))
        if a.phase == b.phase:
            phase_match += 1
        if phase_bucket(a.phase) == phase_bucket(b.phase):
            bucket_match += 1

        phase_counts_legacy[a.phase] = phase_counts_legacy.get(a.phase, 0) + 1
        phase_counts_rusty[b.phase] = phase_counts_rusty.get(b.phase, 0) + 1
        transition_counts[(a.phase, b.phase)] = transition_counts.get((a.phase, b.phase), 0) + 1
        legacy_phases.add(a.phase)
        rusty_phases.add(b.phase)

    phase_match_ratio = phase_match / len(keys)
    bucket_match_ratio = bucket_match / len(keys)

    print("Choke Trace Comparison")
    print(f"  overlap rows: {len(keys)}")
    print(f"  phase match: {phase_match_ratio:.3f}")
    print(f"  bucket match (free/active): {bucket_match_ratio:.3f}")
    print(f"  coherence MAE: {mean_abs_err(coh_err):.6f}")
    print(f"  energy MAE: {mean_abs_err(en_err):.6f}")

    print("  legacy phase counts:")
    for p in sorted(phase_counts_legacy):
        print(f"    {p}: {phase_counts_legacy[p]}")

    print("  rustypi phase counts:")
    for p in sorted(phase_counts_rusty):
        print(f"    {p}: {phase_counts_rusty[p]}")

    legacy_labels = sorted(legacy_phases)
    rusty_labels = sorted(rusty_phases)
    row_label_width = max(len("legacy\\rustypi"), *(len(p) for p in legacy_labels))
    col_widths = {
        p: max(len(p), len(str(max(transition_counts.get((lp, p), 0) for lp in legacy_labels))))
        for p in rusty_labels
    }

    print("  transition matrix (legacy -> rustypi):")
    header = "    " + "legacy\\rustypi".ljust(row_label_width)
    for p in rusty_labels:
        header += "  " + p.rjust(col_widths[p])
    print(header)
    for lp in legacy_labels:
        row = "    " + lp.ljust(row_label_width)
        for rp in rusty_labels:
            row += "  " + str(transition_counts.get((lp, rp), 0)).rjust(col_widths[rp])
        print(row)

    mismatches: list[tuple[int, str, str]] = []
    for (lp, rp), count in transition_counts.items():
        if lp != rp:
            mismatches.append((count, lp, rp))
    mismatches.sort(reverse=True)
    print("  top mismatches (legacy -> rustypi):")
    for count, lp, rp in mismatches[:10]:
        print(f"    {lp} -> {rp}: {count}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
