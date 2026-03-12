#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
from pathlib import Path

from run_benchmark_battery import compute_metrics, evaluate, load_rows, run_trace


def parse_int_list(s: str) -> list[int]:
    return [int(x.strip()) for x in s.split(",") if x.strip()]


def parse_str_list(s: str) -> list[str]:
    return [x.strip() for x in s.split(",") if x.strip()]


def main() -> int:
    ap = argparse.ArgumentParser(description="Generate a limit heatmap CSV from benchmark battery sweeps")
    ap.add_argument("--repo", default=".")
    ap.add_argument("--ticks", type=int, default=512)
    ap.add_argument("--nodes", type=int, default=24)
    ap.add_argument("--channels", default="trap,radiative")
    ap.add_argument("--targets", default="192,224,256,288,320")
    ap.add_argument("--depths", default="3,4")
    ap.add_argument("--calms", default="35,40,50")
    ap.add_argument("--out", default="reports/limit_heatmap.csv")
    args = ap.parse_args()

    repo = Path(args.repo).resolve()
    out_path = (repo / args.out).resolve()
    out_path.parent.mkdir(parents=True, exist_ok=True)

    channels = parse_str_list(args.channels)
    targets = parse_int_list(args.targets)
    depths = parse_int_list(args.depths)
    calms = parse_int_list(args.calms)

    rows: list[dict[str, str]] = []

    for channel in channels:
        for target in targets:
            for depth in depths:
                for calm in calms:
                    trace_name = f"tmp_heatmap_{channel}_t{target}_d{depth}_c{calm}.csv"
                    trace_path = repo / trace_name

                    run_trace(
                        repo=repo,
                        out_csv=trace_path,
                        ticks=args.ticks,
                        nodes=args.nodes,
                        channel=channel,
                        target=target,
                        depth=depth,
                        calm=calm,
                    )

                    metrics = compute_metrics(load_rows(trace_path))
                    checks = evaluate(metrics)
                    pass_count = sum(1 for _, ok, _ in checks if ok)

                    rows.append(
                        {
                            "channel": channel,
                            "target": str(target),
                            "depth": str(depth),
                            "calm_pct": str(calm),
                            "pass_count": str(pass_count),
                            "mean_compression_index": f"{metrics['mean_compression_index']:.6f}",
                            "nonzero_compression_ratio": f"{metrics['nonzero_compression_ratio']:.6f}",
                            "anchor_retention_gain": f"{metrics['anchor_retention_gain']:.6f}",
                            "dissolution_ratio": f"{metrics['dissolution_ratio']:.6f}",
                            "catastrophic_pathway_ratio": f"{metrics['catastrophic_pathway_ratio']:.6f}",
                            "unresolved_ringdown_ratio": f"{metrics['unresolved_ringdown_ratio']:.6f}",
                        }
                    )

                    trace_path.unlink(missing_ok=True)

    rows.sort(
        key=lambda r: (
            r["channel"],
            int(r["depth"]),
            int(r["calm_pct"]),
            int(r["target"]),
        )
    )

    with out_path.open("w", encoding="utf-8", newline="") as f:
        writer = csv.DictWriter(
            f,
            fieldnames=[
                "channel",
                "target",
                "depth",
                "calm_pct",
                "pass_count",
                "mean_compression_index",
                "nonzero_compression_ratio",
                "anchor_retention_gain",
                "dissolution_ratio",
                "catastrophic_pathway_ratio",
                "unresolved_ringdown_ratio",
            ],
        )
        writer.writeheader()
        writer.writerows(rows)

    print("Limit Heatmap")
    print(f"  rows: {len(rows)}")
    print(f"  out: {out_path}")

    # Quick frontier summary: max target with full pass for each channel/depth/calm tuple.
    print("  frontier (max target with 7/7 pass):")
    key_best: dict[tuple[str, str, str], int] = {}
    for r in rows:
        if int(r["pass_count"]) < 7:
            continue
        key = (r["channel"], r["depth"], r["calm_pct"])
        target = int(r["target"])
        if key not in key_best or target > key_best[key]:
            key_best[key] = target

    if not key_best:
        print("    none")
    else:
        for key in sorted(key_best):
            ch, d, c = key
            print(f"    channel={ch} depth={d} calm={c} -> max_target={key_best[key]}")

    return 0


if __name__ == "__main__":
    raise SystemExit(main())
