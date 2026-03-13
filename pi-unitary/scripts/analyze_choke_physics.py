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


def analyze(rows: list[dict]) -> dict:
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

    out_md.write_text("\n".join(lines) + "\n", encoding="utf-8")


def main() -> int:
    ap = argparse.ArgumentParser(description="Analyze decoded choke CSV for physics health")
    ap.add_argument("--csv", required=True, help="Decoded choke CSV path")
    ap.add_argument(
        "--out-prefix",
        default=None,
        help="Output path prefix (without extension). Defaults to <csv_dir>/physics_report",
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
    result = analyze(rows)

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
