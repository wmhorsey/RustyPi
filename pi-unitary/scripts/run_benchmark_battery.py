#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import subprocess
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
    pathway: str


def clamp_non_negative(x: float) -> float:
    return x if x > 0.0 else 0.0


def compression_index(r: Row) -> float:
    return clamp_non_negative(r.coherence) + clamp_non_negative(r.energy)


def shell_tension(r: Row) -> float:
    c = clamp_non_negative(r.coherence)
    e = clamp_non_negative(r.energy)
    return c if c < e else e


def load_rows(path: Path) -> dict[int, list[Row]]:
    by_node: dict[int, list[Row]] = {}
    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        required = {"tick", "node", "drive", "coherence", "energy", "phase"}
        missing = required - set(reader.fieldnames or [])
        if missing:
            raise ValueError(f"{path}: missing columns {sorted(missing)}")

        has_pathway = "pathway" in set(reader.fieldnames or [])

        for rec in reader:
            row = Row(
                tick=int(rec["tick"]),
                node=int(rec["node"]),
                drive=int(float(rec["drive"])),
                coherence=float(rec["coherence"]),
                energy=float(rec["energy"]),
                phase=(rec.get("phase") or "").strip().lower(),
                pathway=((rec.get("pathway") or "").strip().lower() if has_pathway else ""),
            )
            if row.node not in by_node:
                by_node[row.node] = []
            by_node[row.node].append(row)

    for rows in by_node.values():
        rows.sort(key=lambda r: r.tick)
    return by_node


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


def detect_spikes(rows: list[Row]) -> list[int]:
    out: list[int] = []
    prev = 0
    for r in rows:
        if r.drive > 0 and prev <= 0:
            out.append(r.tick)
        prev = r.drive
    return out


def compute_metrics(by_node: dict[int, list[Row]]) -> dict[str, float]:
    total = 0
    ci_sum = 0.0
    nonzero_ci = 0
    dissolution = 0
    catastrophic = 0

    anchor_window = 6
    retain_tick = 16
    anchored_vals: list[float] = []
    unanchored_vals: list[float] = []

    half_lives: list[int] = []
    unresolved = 0
    spike_count = 0

    for rows in by_node.values():
        for r in rows:
            total += 1
            ci = compression_index(r)
            ci_sum += ci
            if ci > 0.0:
                nonzero_ci += 1
            if r.phase == "dissolution":
                dissolution += 1
            if r.pathway == "catastrophic_collapse":
                catastrophic += 1

        spikes = detect_spikes(rows)
        spike_count += len(spikes)

        for t0 in spikes:
            # ringdown
            base = row_at_tick(rows, t0)
            if base is None:
                continue
            base_ci = compression_index(base)
            peak = base_ci
            peak_tick = t0
            for dt in range(0, 10 + 1):
                rr = row_at_tick(rows, t0 + dt)
                if rr is None:
                    continue
                ci = compression_index(rr)
                if ci > peak:
                    peak = ci
                    peak_tick = rr.tick
            target = base_ci + (peak - base_ci) * 0.5
            found = False
            for dt in range(0, 48 + 1):
                rr = row_at_tick(rows, peak_tick + dt)
                if rr is None:
                    continue
                if compression_index(rr) <= target:
                    half_lives.append(dt)
                    found = True
                    break
            if not found:
                unresolved += 1

            # anchoring proxy
            entered_shell = False
            for dt in range(0, anchor_window + 1):
                rr = row_at_tick(rows, t0 + dt)
                if rr is None:
                    continue
                if rr.phase in ACTIVE_PHASES:
                    entered_shell = True
                    break
            rr_later = row_at_tick(rows, t0 + retain_tick)
            if rr_later is None:
                continue
            ret = compression_index(rr_later)
            if entered_shell:
                anchored_vals.append(ret)
            else:
                unanchored_vals.append(ret)

    mean_ci = (ci_sum / total) if total else 0.0
    nonzero_ratio = (nonzero_ci / total) if total else 0.0
    dissolution_ratio = (dissolution / total) if total else 0.0
    catastrophic_ratio = (catastrophic / total) if total else 0.0
    anchor_gain = (sum(anchored_vals) / len(anchored_vals) if anchored_vals else 0.0) - (
        sum(unanchored_vals) / len(unanchored_vals) if unanchored_vals else 0.0
    )
    median_half = sorted(half_lives)[len(half_lives) // 2] if half_lives else 0.0
    unresolved_ratio = (unresolved / spike_count) if spike_count else 0.0

    return {
        "mean_compression_index": mean_ci,
        "nonzero_compression_ratio": nonzero_ratio,
        "anchor_retention_gain": anchor_gain,
        "median_half_life_ticks": float(median_half),
        "unresolved_ringdown_ratio": unresolved_ratio,
        "dissolution_ratio": dissolution_ratio,
        "catastrophic_pathway_ratio": catastrophic_ratio,
        "spike_count": float(spike_count),
    }


def run_trace(repo: Path, out_csv: Path, ticks: int, nodes: int, channel: str, target: int, depth: int, calm: int) -> None:
    cmd = [
        "cargo",
        "run",
        "--quiet",
        "--bin",
        "choke_trace",
        "--",
        "--steps",
        str(ticks),
        "--nodes",
        str(nodes),
        "--channel",
        channel,
        "--target",
        str(target),
        "--generation-depth",
        str(depth),
        "--calm-pct",
        str(calm),
        "--out",
        str(out_csv),
    ]
    subprocess.run(cmd, cwd=repo, check=True)


def evaluate(metrics: dict[str, float]) -> list[tuple[str, bool, str]]:
    checks: list[tuple[str, bool, str]] = []

    checks.append((
        "Law1.mean_compression_index>=0.5",
        metrics["mean_compression_index"] >= 0.5,
        f"{metrics['mean_compression_index']:.6f}",
    ))
    checks.append((
        "Law1.nonzero_compression_ratio>=0.10",
        metrics["nonzero_compression_ratio"] >= 0.10,
        f"{metrics['nonzero_compression_ratio']:.6f}",
    ))
    checks.append((
        "Law1.anchor_retention_gain>0",
        metrics["anchor_retention_gain"] > 0.0,
        f"{metrics['anchor_retention_gain']:.6f}",
    ))

    checks.append((
        "Law2.unresolved_ringdown_ratio<=0.10",
        metrics["unresolved_ringdown_ratio"] <= 0.10,
        f"{metrics['unresolved_ringdown_ratio']:.6f}",
    ))
    checks.append((
        "Law2.median_half_life_ticks<=8",
        metrics["median_half_life_ticks"] <= 8.0,
        f"{metrics['median_half_life_ticks']:.6f}",
    ))

    checks.append((
        "Law3.dissolution_ratio<=0.10",
        metrics["dissolution_ratio"] <= 0.10,
        f"{metrics['dissolution_ratio']:.6f}",
    ))
    checks.append((
        "Law3.catastrophic_pathway_ratio<=0.02",
        metrics["catastrophic_pathway_ratio"] <= 0.02,
        f"{metrics['catastrophic_pathway_ratio']:.6f}",
    ))

    return checks


def main() -> int:
    ap = argparse.ArgumentParser(description="Run benchmark battery for falsifiable STE laws")
    ap.add_argument("--repo", default=".")
    ap.add_argument("--ticks", type=int, default=1024)
    ap.add_argument("--nodes", type=int, default=32)
    ap.add_argument("--channel", default="trap")
    ap.add_argument("--target", type=int, default=128)
    ap.add_argument("--generation-depth", type=int, default=2)
    ap.add_argument("--calm-pct", type=int, default=70)
    ap.add_argument("--out-trace", default="reports/benchmark_trace.csv")
    args = ap.parse_args()

    repo = Path(args.repo).resolve()
    out_trace = (repo / args.out_trace).resolve()
    out_trace.parent.mkdir(parents=True, exist_ok=True)

    run_trace(
        repo,
        out_trace,
        args.ticks,
        args.nodes,
        args.channel,
        args.target,
        args.generation_depth,
        args.calm_pct,
    )

    metrics = compute_metrics(load_rows(out_trace))
    checks = evaluate(metrics)
    passed = sum(1 for _, ok, _ in checks if ok)

    print("Benchmark Battery")
    print(
        f"  scenario: channel={args.channel} target={args.target} depth={args.generation_depth} calm={args.calm_pct}"
    )
    print(f"  trace: {out_trace}")
    print("  metrics:")
    for k in [
        "mean_compression_index",
        "nonzero_compression_ratio",
        "anchor_retention_gain",
        "median_half_life_ticks",
        "unresolved_ringdown_ratio",
        "dissolution_ratio",
        "catastrophic_pathway_ratio",
        "spike_count",
    ]:
        print(f"    {k}: {metrics[k]:.6f}")

    print("  checks:")
    for name, ok, val in checks:
        status = "PASS" if ok else "FAIL"
        print(f"    [{status}] {name} ({val})")

    print(f"  summary: {passed}/{len(checks)} checks passed")
    return 0 if passed == len(checks) else 1


if __name__ == "__main__":
    raise SystemExit(main())
