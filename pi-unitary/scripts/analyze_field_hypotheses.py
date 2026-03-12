#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import statistics
from dataclasses import dataclass
from pathlib import Path

ACTIVE_PHASES = {"formation", "liftoff", "coherence", "drift", "dissolution"}


@dataclass(frozen=True)
class Row:
    tick: int
    node: int
    drive: int
    coherence: float
    energy: float
    phase: str


@dataclass(frozen=True)
class SpikeEvent:
    node: int
    tick: int


def clamp_non_negative(x: float) -> float:
    return x if x > 0.0 else 0.0


def shell_tension(row: Row) -> float:
    c = clamp_non_negative(row.coherence)
    e = clamp_non_negative(row.energy)
    return c if c < e else e


def compression_index(row: Row) -> float:
    c = clamp_non_negative(row.coherence)
    e = clamp_non_negative(row.energy)
    return c + e


def load_trace(path: Path) -> dict[int, list[Row]]:
    out: dict[int, list[Row]] = {}
    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        required = {"tick", "node", "drive", "coherence", "energy", "phase"}
        missing = required - set(reader.fieldnames or [])
        if missing:
            raise ValueError(f"{path}: missing columns {sorted(missing)}")

        for rec in reader:
            row = Row(
                tick=int(rec["tick"]),
                node=int(rec["node"]),
                drive=int(float(rec["drive"])),
                coherence=float(rec["coherence"]),
                energy=float(rec["energy"]),
                phase=(rec["phase"] or "").strip().lower(),
            )
            if row.node not in out:
                out[row.node] = []
            out[row.node].append(row)

    for rows in out.values():
        rows.sort(key=lambda r: r.tick)
    return out


def contiguous_windows(rows: list[Row], predicate) -> list[list[Row]]:
    windows: list[list[Row]] = []
    cur: list[Row] = []
    for r in rows:
        if predicate(r):
            cur.append(r)
        elif cur:
            windows.append(cur)
            cur = []
    if cur:
        windows.append(cur)
    return windows


def analyze_unperturbed_stability(by_node: dict[int, list[Row]]) -> dict[str, float]:
    slopes: list[float] = []
    lengths: list[int] = []
    surviving_windows = 0
    total_windows = 0

    for rows in by_node.values():
        windows = contiguous_windows(rows, lambda r: r.drive == 0 and r.phase in ACTIVE_PHASES)
        for w in windows:
            if len(w) < 5:
                continue
            total_windows += 1
            t0 = compression_index(w[0])
            t1 = compression_index(w[-1])
            slope = (t1 - t0) / max(1, (len(w) - 1))
            slopes.append(slope)
            lengths.append(len(w))
            if t1 >= t0 * 0.8:
                surviving_windows += 1

    return {
        "window_count": float(total_windows),
        "mean_window_len": statistics.mean(lengths) if lengths else 0.0,
        "mean_compression_slope_per_tick": statistics.mean(slopes) if slopes else 0.0,
        "stable_window_ratio": (surviving_windows / total_windows) if total_windows else 0.0,
    }


def detect_spikes(rows: list[Row]) -> list[SpikeEvent]:
    events: list[SpikeEvent] = []
    prev_drive = 0
    for r in rows:
        if r.drive > 0 and prev_drive <= 0:
            events.append(SpikeEvent(node=r.node, tick=r.tick))
        prev_drive = r.drive
    return events


def row_at_tick(rows: list[Row], tick: int) -> Row | None:
    lo = 0
    hi = len(rows) - 1
    while lo <= hi:
        mid = (lo + hi) // 2
        t = rows[mid].tick
        if t == tick:
            return rows[mid]
        if t < tick:
            lo = mid + 1
        else:
            hi = mid - 1
    return None


def analyze_spike_ringdown(by_node: dict[int, list[Row]]) -> dict[str, float]:
    half_lives: list[int] = []
    unresolved = 0
    total = 0

    peak_horizon = 10
    decay_horizon = 48

    for node, rows in by_node.items():
        for ev in detect_spikes(rows):
            total += 1
            base_row = row_at_tick(rows, ev.tick)
            if base_row is None:
                continue
            base = compression_index(base_row)

            peak = base
            peak_tick = ev.tick
            for dt in range(0, peak_horizon + 1):
                rr = row_at_tick(rows, ev.tick + dt)
                if rr is None:
                    continue
                t = compression_index(rr)
                if t > peak:
                    peak = t
                    peak_tick = rr.tick

            target = base + (peak - base) * 0.5
            found = False
            for dt in range(0, decay_horizon + 1):
                rr = row_at_tick(rows, peak_tick + dt)
                if rr is None:
                    continue
                if compression_index(rr) <= target:
                    half_lives.append(dt)
                    found = True
                    break

            if not found:
                unresolved += 1

    return {
        "spike_count": float(total),
        "median_half_life_ticks": statistics.median(half_lives) if half_lives else 0.0,
        "mean_half_life_ticks": statistics.mean(half_lives) if half_lives else 0.0,
        "unresolved_ringdown_ratio": (unresolved / total) if total else 0.0,
    }


def analyze_depression_anchor(by_node: dict[int, list[Row]]) -> dict[str, float]:
    anchor_window = 6
    retention_tick = 16

    anchored_retention: list[float] = []
    unanchored_retention: list[float] = []

    for rows in by_node.values():
        for ev in detect_spikes(rows):
            at_spike = row_at_tick(rows, ev.tick)
            at_later = row_at_tick(rows, ev.tick + retention_tick)
            if at_spike is None or at_later is None:
                continue

            entered_shell = False
            for dt in range(0, anchor_window + 1):
                rr = row_at_tick(rows, ev.tick + dt)
                if rr is None:
                    continue
                if rr.phase in ACTIVE_PHASES:
                    entered_shell = True
                    break

            retention = compression_index(at_later)
            if entered_shell:
                anchored_retention.append(retention)
            else:
                unanchored_retention.append(retention)

    anchored_mean = statistics.mean(anchored_retention) if anchored_retention else 0.0
    unanchored_mean = statistics.mean(unanchored_retention) if unanchored_retention else 0.0

    return {
        "anchored_events": float(len(anchored_retention)),
        "unanchored_events": float(len(unanchored_retention)),
        "anchored_mean_compression_t_plus_16": anchored_mean,
        "unanchored_mean_compression_t_plus_16": unanchored_mean,
        "anchor_retention_gain": anchored_mean - unanchored_mean,
    }


def analyze_field_load(by_node: dict[int, list[Row]]) -> dict[str, float]:
    values: list[float] = []
    active = 0
    total = 0
    for rows in by_node.values():
        for r in rows:
            ci = compression_index(r)
            values.append(ci)
            total += 1
            if ci > 0.0:
                active += 1

    mean_ci = statistics.mean(values) if values else 0.0
    p95_ci = statistics.quantiles(values, n=20)[18] if len(values) >= 20 else mean_ci
    active_ratio = (active / total) if total else 0.0

    # Heuristic: below this, traces are too unloaded to support boundary-anchoring claims.
    load_ok = 1.0 if (mean_ci >= 0.5 and active_ratio >= 0.1) else 0.0
    return {
        "mean_compression_index": mean_ci,
        "p95_compression_index": p95_ci,
        "nonzero_compression_ratio": active_ratio,
        "load_valid_for_anchor_tests": load_ok,
    }


def print_block(title: str, stats: dict[str, float]) -> None:
    print(title)
    for k, v in stats.items():
        if k.endswith("ratio"):
            print(f"  {k}: {v:.3f}")
        else:
            print(f"  {k}: {v:.6f}" if isinstance(v, float) else f"  {k}: {v}")


def main() -> int:
    ap = argparse.ArgumentParser(description="Analyze whether trace data supports core field hypotheses")
    ap.add_argument("--trace", required=True, help="RustyPi choke trace CSV path")
    args = ap.parse_args()

    by_node = load_trace(Path(args.trace))

    load = analyze_field_load(by_node)
    stability = analyze_unperturbed_stability(by_node)
    ringdown = analyze_spike_ringdown(by_node)
    anchoring = analyze_depression_anchor(by_node)

    print("Field Hypothesis Analysis")
    print_block("Field Load Diagnostics", load)
    print_block("Unperturbed Boundary Stability", stability)
    print_block("Spike Ring-Down", ringdown)
    print_block("Depression Anchor", anchoring)

    if load["load_valid_for_anchor_tests"] < 1.0:
        print("Interpretation")
        print("  low-load regime detected: anchoring conclusions are provisional")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
