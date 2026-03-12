#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import subprocess
from dataclasses import dataclass
from pathlib import Path

PHASES = ["free", "formation", "liftoff", "coherence", "drift", "dissolution"]


@dataclass
class ScoreRow:
    channel: str
    target: int
    mean_ci: float
    nonzero_ratio: float
    active_ratio: float
    free_ratio: float
    dissolution_ratio: float
    score: float


def load_metrics(path: Path) -> tuple[float, float, float, float, float]:
    total = 0
    nonzero_ci = 0
    active = 0
    free = 0
    dissolution = 0
    ci_sum = 0.0

    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        for rec in reader:
            total += 1
            coherence = float(rec.get("coherence", 0.0))
            energy = float(rec.get("energy", 0.0))
            phase = (rec.get("phase") or "").strip().lower()
            ci = max(0.0, coherence) + max(0.0, energy)
            ci_sum += ci
            if ci > 0.0:
                nonzero_ci += 1
            if phase != "free":
                active += 1
            if phase == "free":
                free += 1
            if phase == "dissolution":
                dissolution += 1

    if total == 0:
        return 0.0, 0.0, 0.0, 0.0, 0.0

    mean_ci = ci_sum / total
    return (
        mean_ci,
        nonzero_ci / total,
        active / total,
        free / total,
        dissolution / total,
    )


def score(mean_ci: float, active_ratio: float, dissolution_ratio: float) -> float:
    # Target a balanced loaded regime with low catastrophic dissolution.
    ci_term = 1.0 - abs(mean_ci - 0.5) / 0.5
    if ci_term < 0.0:
        ci_term = 0.0

    active_term = 1.0 - abs(active_ratio - 0.25) / 0.25
    if active_term < 0.0:
        active_term = 0.0

    dissolution_term = 1.0 - min(1.0, dissolution_ratio / 0.08)

    return 0.45 * ci_term + 0.35 * active_term + 0.20 * dissolution_term


def run_trace(repo: Path, channel: str, target: int, out_csv: Path, ticks: int, nodes: int) -> None:
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
        "--out",
        str(out_csv),
    ]
    subprocess.run(cmd, cwd=repo, check=True)


def main() -> int:
    ap = argparse.ArgumentParser(description="Sweep channel/target settings to find a balanced reality range")
    ap.add_argument("--repo", default=".", help="pi-unitary repo root")
    ap.add_argument("--ticks", type=int, default=1024)
    ap.add_argument("--nodes", type=int, default=32)
    ap.add_argument("--targets", default="0,1,2,4,8,16,32,64", help="comma-separated target ticks")
    ap.add_argument("--channels", default="trap,radiative", help="comma-separated channels")
    ap.add_argument("--out", default="reports/reality_range_summary.csv", help="summary CSV output")
    args = ap.parse_args()

    repo = Path(args.repo).resolve()
    out_path = (repo / args.out).resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)

    targets = [int(x.strip()) for x in args.targets.split(",") if x.strip()]
    channels = [x.strip() for x in args.channels.split(",") if x.strip()]

    rows: list[ScoreRow] = []

    for channel in channels:
        for target in targets:
            trace_path = repo / f"tmp_trace_{channel}_{target}.csv"
            run_trace(repo, channel, target, trace_path, args.ticks, args.nodes)
            mean_ci, nonzero_ratio, active_ratio, free_ratio, dissolution_ratio = load_metrics(trace_path)
            s = score(mean_ci, active_ratio, dissolution_ratio)
            rows.append(
                ScoreRow(
                    channel=channel,
                    target=target,
                    mean_ci=mean_ci,
                    nonzero_ratio=nonzero_ratio,
                    active_ratio=active_ratio,
                    free_ratio=free_ratio,
                    dissolution_ratio=dissolution_ratio,
                    score=s,
                )
            )
            trace_path.unlink(missing_ok=True)

    rows.sort(key=lambda r: r.score, reverse=True)

    with out_path.open("w", encoding="utf-8", newline="") as f:
        writer = csv.writer(f)
        writer.writerow(
            [
                "channel",
                "target",
                "mean_ci",
                "nonzero_ci_ratio",
                "active_ratio",
                "free_ratio",
                "dissolution_ratio",
                "score",
            ]
        )
        for r in rows:
            writer.writerow(
                [
                    r.channel,
                    r.target,
                    f"{r.mean_ci:.6f}",
                    f"{r.nonzero_ratio:.6f}",
                    f"{r.active_ratio:.6f}",
                    f"{r.free_ratio:.6f}",
                    f"{r.dissolution_ratio:.6f}",
                    f"{r.score:.6f}",
                ]
            )

    print("Reality Range Sweep")
    print(f"  rows: {len(rows)}")
    print(f"  summary: {out_path}")
    print("  top 5:")
    for r in rows[:5]:
        print(
            f"    {r.channel:9s} target={r.target:3d} score={r.score:.3f} "
            f"mean_ci={r.mean_ci:.3f} active={r.active_ratio:.3f} dissolution={r.dissolution_ratio:.3f}"
        )

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
