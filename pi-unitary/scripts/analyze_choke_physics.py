#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import json
from collections import Counter, defaultdict
from dataclasses import dataclass
from pathlib import Path

TAU_TICKS_DEFAULT = 4096

PHASE_TO_ID = {
    "free": 0,
    "formation": 1,
    "liftoff": 2,
    "coherence": 3,
    "drift": 4,
    "dissolution": 5,
}

PHASE_TO_PATHWAY = {
    "free": "free_pool",
    "formation": "depression_consumption",
    "liftoff": "depression_consumption",
    "coherence": "choke_shell_structuring",
    "drift": "choke_shell_structuring",
}

PATHWAY_TO_ID = {
    "free_pool": 0,
    "depression_consumption": 1,
    "choke_shell_structuring": 2,
    "radiative_release": 3,
    "catastrophic_collapse": 4,
}


@dataclass
class CheckResult:
    name: str
    ok: bool
    detail: str


def expected_pathway(phase: str, energy: int, coherence: int) -> str:
    if phase == "dissolution":
        return "catastrophic_collapse" if energy > coherence else "radiative_release"
    return PHASE_TO_PATHWAY.get(phase, "unknown")


def load_rows(csv_path: Path) -> list[dict]:
    with csv_path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        required = {
            "tick",
            "sequence",
            "node",
            "phase_tick",
            "coherence",
            "energy",
            "shell_ring_ticks",
            "spin_bias",
            "phase_id",
            "phase",
            "pathway_id",
            "pathway",
            "drive",
        }
        missing = required - set(reader.fieldnames or [])
        if missing:
            raise ValueError(f"missing columns: {sorted(missing)}")

        rows: list[dict] = []
        for rec in reader:
            rows.append(
                {
                    "tick": int(rec["tick"]),
                    "sequence": int(rec["sequence"]),
                    "node": int(rec["node"]),
                    "phase_tick": int(rec["phase_tick"]),
                    "coherence": int(rec["coherence"]),
                    "energy": int(rec["energy"]),
                    "shell_ring_ticks": int(rec["shell_ring_ticks"]),
                    "spin_bias": int(rec["spin_bias"]),
                    "phase_id": int(rec["phase_id"]),
                    "phase": (rec["phase"] or "").strip().lower(),
                    "pathway_id": int(rec["pathway_id"]),
                    "pathway": (rec["pathway"] or "").strip().lower(),
                    "drive": int(rec["drive"]),
                }
            )
    return rows


def analyze(rows: list[dict], boundary_rate_num: int | None, boundary_rate_den: int | None) -> dict:
    checks: list[CheckResult] = []
    warnings: list[str] = []

    checks.append(CheckResult("csv not empty", len(rows) > 0, f"rows={len(rows)}"))
    if not rows:
        return {
            "ok": False,
            "checks": [c.__dict__ for c in checks],
            "warnings": warnings,
            "summary": {},
        }

    by_tick: dict[int, list[dict]] = defaultdict(list)
    dup_count = 0
    seen = set()
    neg_energy = 0
    neg_coherence = 0
    bad_phase_id = 0
    bad_pathway = 0
    bad_pathway_id = 0
    tau_violations = 0
    floor_hits_energy = 0
    floor_hits_coherence = 0
    floor_hits_shell = 0
    floor_hits_spin = 0

    for r in rows:
        key = (r["tick"], r["sequence"], r["node"])
        if key in seen:
            dup_count += 1
        seen.add(key)

        if r["energy"] < 0:
            neg_energy += 1
        if r["coherence"] < 0:
            neg_coherence += 1

        expect_phase_id = PHASE_TO_ID.get(r["phase"])
        if expect_phase_id is None or r["phase_id"] != expect_phase_id:
            bad_phase_id += 1

        expect_pathway = expected_pathway(r["phase"], r["energy"], r["coherence"])
        if r["pathway"] != expect_pathway:
            bad_pathway += 1

        expect_pathway_id = PATHWAY_TO_ID.get(expect_pathway)
        if expect_pathway_id is None or r["pathway_id"] != expect_pathway_id:
            bad_pathway_id += 1

        if r["phase_tick"] < 0 or r["phase_tick"] >= TAU_TICKS_DEFAULT:
            tau_violations += 1

        if r["energy"] == 0:
            floor_hits_energy += 1
        if r["coherence"] == 0:
            floor_hits_coherence += 1
        if r["shell_ring_ticks"] == 0:
            floor_hits_shell += 1
        if r["spin_bias"] == 0:
            floor_hits_spin += 1

        by_tick[r["tick"]].append(r)

    tick_counts = {tick: len(grp) for tick, grp in by_tick.items()}
    min_nodes = min(tick_counts.values())
    max_nodes = max(tick_counts.values())

    checks.append(
        CheckResult(
            "no duplicate (tick,sequence,node)",
            dup_count == 0,
            f"duplicates={dup_count}",
        )
    )
    checks.append(
        CheckResult(
            "constant node count per tick",
            min_nodes == max_nodes,
            f"min={min_nodes}, max={max_nodes}, ticks={len(tick_counts)}",
        )
    )
    checks.append(
        CheckResult(
            "non-negative energy/coherence",
            (neg_energy == 0 and neg_coherence == 0),
            f"neg_energy={neg_energy}, neg_coherence={neg_coherence}",
        )
    )
    checks.append(
        CheckResult(
            "phase ids match phase labels",
            bad_phase_id == 0,
            f"mismatches={bad_phase_id}",
        )
    )
    checks.append(
        CheckResult(
            "pathway labels follow phase/energy rules",
            bad_pathway == 0,
            f"mismatches={bad_pathway}",
        )
    )
    checks.append(
        CheckResult(
            "pathway ids match pathway labels",
            bad_pathway_id == 0,
            f"mismatches={bad_pathway_id}",
        )
    )

    if tau_violations > 0:
        warnings.append(
            f"phase_tick outside [0,{TAU_TICKS_DEFAULT - 1}] seen {tau_violations} time(s)"
        )

    ticks_sorted = sorted(by_tick.keys())
    first_tick = ticks_sorted[0]
    last_tick = ticks_sorted[-1]
    first_phase = Counter(r["phase"] for r in by_tick[first_tick])
    last_phase = Counter(r["phase"] for r in by_tick[last_tick])
    all_phase = Counter(r["phase"] for r in rows)

    # Boundary diagnostics across node trajectories.
    by_node_tick: dict[int, list[tuple[int, int]]] = defaultdict(list)
    for r in rows:
        by_node_tick[r["node"]].append((r["tick"], r["phase_tick"]))
    wrap_events = 0
    delta_sum = 0
    delta_count = 0
    for seq in by_node_tick.values():
        seq.sort(key=lambda x: x[0])
        for idx in range(1, len(seq)):
            prev_pt = seq[idx - 1][1]
            cur_pt = seq[idx][1]
            delta = cur_pt - prev_pt
            if delta < 0:
                delta += TAU_TICKS_DEFAULT
            delta_sum += delta
            delta_count += 1
            if cur_pt < prev_pt:
                wrap_events += 1

    tick_diffs = [
        ticks_sorted[i] - ticks_sorted[i - 1] for i in range(1, len(ticks_sorted))
    ]
    tick_stride = 1
    if tick_diffs:
        # Mode of step size gives robust snapshot cadence.
        tick_stride = Counter(tick_diffs).most_common(1)[0][0]

    avg_phase_tick_delta_per_snapshot = (
        (delta_sum / delta_count) if delta_count > 0 else 0.0
    )
    effective_field_rate_per_tick = (
        avg_phase_tick_delta_per_snapshot / float(tick_stride)
        if tick_stride > 0
        else 0.0
    )
    inferred_boundary_tension = (
        (1.0 / effective_field_rate_per_tick)
        if effective_field_rate_per_tick > 0
        else None
    )

    residual_abs_total: int | None = None
    residual_max_abs: int | None = None
    residual_samples: int | None = None
    if boundary_rate_num is not None and boundary_rate_den is not None:
        num = max(1, int(boundary_rate_num))
        den = max(1, int(boundary_rate_den))
        abs_sum = 0
        max_abs = 0
        samples = 0
        for node, seq in by_node_tick.items():
            seq.sort(key=lambda x: x[0])
            for idx in range(1, len(seq)):
                t0, pt0 = seq[idx - 1]
                t1, pt1 = seq[idx]
                actual = pt1 - pt0
                if actual < 0:
                    actual += TAU_TICKS_DEFAULT
                # Snapshots are recorded after completing simulation step t,
                # and the kernel step uses x0 = (t + 1) + node with a lookahead
                # to x0 + 1. This telescopes to +2 alignment at snapshot time.
                x0 = t0 + node + 2
                x1 = t1 + node + 2
                expected = (x1 * num) // den - (x0 * num) // den
                err = actual - expected
                aerr = abs(err)
                abs_sum += aerr
                if aerr > max_abs:
                    max_abs = aerr
                samples += 1
        residual_abs_total = abs_sum
        residual_max_abs = max_abs
        residual_samples = samples

    total_rows = len(rows)
    floor_share = {
        "energy_zero_share": floor_hits_energy / total_rows,
        "coherence_zero_share": floor_hits_coherence / total_rows,
        "shell_zero_share": floor_hits_shell / total_rows,
        "spin_zero_share": floor_hits_spin / total_rows,
    }

    boundary_profile = {
        "phase_tick_boundary": "periodic_mod_4096",
        "phase_tick_wrap_events": wrap_events,
        "snapshot_tick_stride": tick_stride,
        "avg_phase_tick_delta_per_snapshot": avg_phase_tick_delta_per_snapshot,
        "effective_field_rate_per_tick": effective_field_rate_per_tick,
        "inferred_boundary_tension": inferred_boundary_tension,
        "configured_boundary_rate_num": boundary_rate_num,
        "configured_boundary_rate_den": boundary_rate_den,
        "rational_residual_abs_total": residual_abs_total,
        "rational_residual_max_abs": residual_max_abs,
        "rational_residual_samples": residual_samples,
        "lower_boundaries": {
            "energy": "hard_floor_0",
            "coherence": "hard_floor_0",
            "shell_ring_ticks": "hard_floor_0",
            "spin_bias": "hard_floor_0",
        },
        "floor_shares": floor_share,
    }

    active_first = len(by_tick[first_tick]) - first_phase.get("free", 0)
    active_last = len(by_tick[last_tick]) - last_phase.get("free", 0)

    ok = all(c.ok for c in checks)
    summary = {
        "rows": len(rows),
        "tick_count": len(ticks_sorted),
        "first_tick": first_tick,
        "last_tick": last_tick,
        "nodes_per_tick": max_nodes,
        "active_first": active_first,
        "active_last": active_last,
        "phase_first": dict(first_phase),
        "phase_last": dict(last_phase),
        "phase_total": dict(all_phase),
        "boundary_profile": boundary_profile,
    }

    return {
        "ok": ok,
        "checks": [c.__dict__ for c in checks],
        "warnings": warnings,
        "summary": summary,
    }


def write_markdown(out_md: Path, result: dict, csv_path: Path) -> None:
    status = "PASS" if result["ok"] else "FAIL"
    s = result["summary"]
    lines: list[str] = []
    lines.append("# Choke Physics Health Report")
    lines.append("")
    lines.append(f"- source_csv: {csv_path}")
    lines.append(f"- overall_status: {status}")
    lines.append("")
    lines.append("## What This Means")
    if result["ok"]:
        lines.append(
            "Core ledger invariants hold for this run: accounting is consistent, value domains are valid, and phase/pathway contracts are internally coherent."
        )
    else:
        lines.append(
            "One or more core invariants failed. Treat this run as suspect until the failed checks are resolved."
        )
    lines.append("")
    lines.append("## Checks")
    for c in result["checks"]:
        mark = "PASS" if c["ok"] else "FAIL"
        lines.append(f"- {mark}: {c['name']} ({c['detail']})")
    if result["warnings"]:
        lines.append("")
        lines.append("## Warnings")
        for w in result["warnings"]:
            lines.append(f"- {w}")

    lines.append("")
    lines.append("## Evolution Snapshot")
    lines.append(
        f"- ticks: {s['first_tick']} -> {s['last_tick']} ({s['tick_count']} snapshots)"
    )
    lines.append(f"- nodes_per_tick: {s['nodes_per_tick']}")
    lines.append(f"- active_nodes: {s['active_first']} -> {s['active_last']}")
    lines.append(f"- first_tick_phase_counts: {json.dumps(s['phase_first'], sort_keys=True)}")
    lines.append(f"- last_tick_phase_counts: {json.dumps(s['phase_last'], sort_keys=True)}")
    lines.append("")
    lines.append("## Totals Across Run")
    lines.append(f"- rows: {s['rows']}")
    lines.append(f"- phase_totals: {json.dumps(s['phase_total'], sort_keys=True)}")

    bp = s.get("boundary_profile", {})
    if bp:
        lines.append("")
        lines.append("## Boundary Profile")
        lines.append(f"- phase_tick_boundary: {bp.get('phase_tick_boundary', 'unknown')}")
        lines.append(f"- phase_tick_wrap_events: {bp.get('phase_tick_wrap_events', 0)}")
        lines.append(f"- snapshot_tick_stride: {bp.get('snapshot_tick_stride', 0)}")
        lines.append(
            f"- avg_phase_tick_delta_per_snapshot: {round(float(bp.get('avg_phase_tick_delta_per_snapshot', 0.0)), 4)}"
        )
        lines.append(
            f"- effective_field_rate_per_tick: {round(float(bp.get('effective_field_rate_per_tick', 0.0)), 6)}"
        )
        inf_tension = bp.get("inferred_boundary_tension")
        if inf_tension is None:
            lines.append("- inferred_boundary_tension: n/a")
        else:
            lines.append(f"- inferred_boundary_tension: {round(float(inf_tension), 4)}")
        cfg_num = bp.get("configured_boundary_rate_num")
        cfg_den = bp.get("configured_boundary_rate_den")
        if cfg_num is not None and cfg_den is not None:
            lines.append(f"- configured_boundary_rate: {cfg_num}/{cfg_den}")
            lines.append(
                f"- rational_residual_abs_total: {bp.get('rational_residual_abs_total', 'n/a')}"
            )
            lines.append(
                f"- rational_residual_max_abs: {bp.get('rational_residual_max_abs', 'n/a')}"
            )
        fs = bp.get("floor_shares", {})
        lines.append(
            "- floor_shares: "
            + json.dumps(
                {
                    "energy_zero_share": round(float(fs.get("energy_zero_share", 0.0)), 4),
                    "coherence_zero_share": round(float(fs.get("coherence_zero_share", 0.0)), 4),
                    "shell_zero_share": round(float(fs.get("shell_zero_share", 0.0)), 4),
                    "spin_zero_share": round(float(fs.get("spin_zero_share", 0.0)), 4),
                },
                sort_keys=True,
            )
        )
        lines.append(
            "- interpretation: lower-state channels are strongly pinned to zero when excitation is not sustained; phase progression wraps on a periodic 4096-tick cycle."
        )

    out_md.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description="Analyze decoded choke CSV for physics health")
    ap.add_argument("--csv", required=True, help="Decoded choke CSV path")
    ap.add_argument(
        "--out-prefix",
        default=None,
        help="Output path prefix (without extension). Defaults to <csv_dir>/physics_report",
    )
    ap.add_argument(
        "--boundary-rate-num",
        type=int,
        default=None,
        help="Optional configured rational boundary numerator for residual checks",
    )
    ap.add_argument(
        "--boundary-rate-den",
        type=int,
        default=None,
        help="Optional configured rational boundary denominator for residual checks",
    )
    args = ap.parse_args()

    csv_path = Path(args.csv)
    if not csv_path.exists():
        raise FileNotFoundError(csv_path)

    if args.out_prefix:
        out_prefix = Path(args.out_prefix)
    else:
        out_prefix = csv_path.parent / "physics_report"

    rows = load_rows(csv_path)
    rate_num = args.boundary_rate_num
    rate_den = args.boundary_rate_den
    if (rate_num is None) ^ (rate_den is None):
        raise ValueError("Provide both --boundary-rate-num and --boundary-rate-den, or neither")

    result = analyze(rows, rate_num, rate_den)

    out_json = out_prefix.with_suffix(".json")
    out_md = out_prefix.with_suffix(".md")
    out_json.write_text(json.dumps(result, indent=2), encoding="utf-8")
    write_markdown(out_md, result, csv_path)

    print(f"overall_status={'PASS' if result['ok'] else 'FAIL'}")
    print(f"report_json={out_json}")
    print(f"report_md={out_md}")
    return 0 if result["ok"] else 2


if __name__ == "__main__":
    raise SystemExit(main())
