#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
from collections import defaultdict
from pathlib import Path

PHASES = ["free", "formation", "liftoff", "coherence", "drift", "dissolution"]
PHASE_COLORS = {
    "free": "#4c78a8",
    "formation": "#f58518",
    "liftoff": "#e45756",
    "coherence": "#72b7b2",
    "drift": "#54a24b",
    "dissolution": "#b279a2",
}


def normalize_phase(s: str) -> str:
    s = (s or "").strip().lower()
    return s


def load_counts_by_tick(path: Path) -> tuple[dict[int, dict[str, int]], int, int]:
    by_tick: dict[int, dict[str, int]] = defaultdict(lambda: defaultdict(int))
    total = 0
    max_tick = 0

    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        required = {"tick", "phase"}
        missing = required - set(reader.fieldnames or [])
        if missing:
            raise ValueError(f"{path}: missing columns {sorted(missing)}")

        for rec in reader:
            tick = int(rec["tick"])
            phase = normalize_phase(rec["phase"])
            by_tick[tick][phase] += 1
            total += 1
            if tick > max_tick:
                max_tick = tick

    return by_tick, total, max_tick


def load_pathway_counts(path: Path) -> dict[str, int]:
    counts: dict[str, int] = defaultdict(int)
    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        if "pathway" not in set(reader.fieldnames or []):
            return {}
        for rec in reader:
            p = (rec.get("pathway") or "").strip().lower()
            if p:
                counts[p] += 1
    return dict(counts)


def build_series(by_tick: dict[int, dict[str, int]], max_tick: int, phase: str) -> list[int]:
    out: list[int] = []
    tick = 0
    while tick <= max_tick:
        out.append(by_tick.get(tick, {}).get(phase, 0))
        tick += 1
    return out


def polyline_points(series: list[int], width: int, height: int) -> str:
    if not series:
        return ""
    max_v = max(series)
    if max_v <= 0:
        max_v = 1

    points: list[str] = []
    n = len(series)
    for i, v in enumerate(series):
        x = int(i * (width - 1) / max(1, n - 1))
        y = int((height - 1) - (v * (height - 1) / max_v))
        points.append(f"{x},{y}")
    return " ".join(points)


def total_phase_counts(by_tick: dict[int, dict[str, int]]) -> dict[str, int]:
    totals: dict[str, int] = defaultdict(int)
    for phase_map in by_tick.values():
        for p, c in phase_map.items():
            totals[p] += c
    return dict(totals)


def render_chart(label: str, by_tick: dict[int, dict[str, int]], max_tick: int) -> str:
    width = 980
    height = 180

    lines: list[str] = []
    for p in PHASES:
        series = build_series(by_tick, max_tick, p)
        points = polyline_points(series, width, height)
        color = PHASE_COLORS[p]
        lines.append(
            f'<polyline points="{points}" fill="none" stroke="{color}" stroke-width="2" />'
        )

    legend = "".join(
        f'<span class="legend-item"><span class="swatch" style="background:{PHASE_COLORS[p]}"></span>{p}</span>'
        for p in PHASES
    )

    return (
        f"<section class='panel'><h3>{label}</h3>"
        f"<div class='legend'>{legend}</div>"
        f"<svg viewBox='0 0 {width} {height}' width='{width}' height='{height}'>{''.join(lines)}</svg>"
        "</section>"
    )


def render_counts_table(legacy_counts: dict[str, int], rusty_counts: dict[str, int]) -> str:
    rows = []
    for p in PHASES:
        lv = legacy_counts.get(p, 0)
        rv = rusty_counts.get(p, 0)
        rows.append(f"<tr><td>{p}</td><td>{lv}</td><td>{rv}</td><td>{rv - lv}</td></tr>")
    return (
        "<table><thead><tr><th>Phase</th><th>Legacy</th><th>RustyPi</th><th>Delta</th></tr></thead>"
        f"<tbody>{''.join(rows)}</tbody></table>"
    )


def render_pathway_table(pathway_counts: dict[str, int]) -> str:
    if not pathway_counts:
        return ""
    keys = sorted(pathway_counts.keys())
    rows = [f"<tr><td>{k}</td><td>{pathway_counts[k]}</td></tr>" for k in keys]
    return (
        "<section class='panel'><h3>RustyPi Pathway Ledger</h3>"
        "<table><thead><tr><th>Pathway</th><th>Count</th></tr></thead>"
        f"<tbody>{''.join(rows)}</tbody></table></section>"
    )


def main() -> int:
    ap = argparse.ArgumentParser(description="Render an HTML report for legacy vs RustyPi choke traces")
    ap.add_argument("--legacy", required=True, help="Legacy CSV path")
    ap.add_argument("--rustypi", required=True, help="RustyPi CSV path")
    ap.add_argument("--out", required=True, help="Output HTML path")
    args = ap.parse_args()

    legacy_path = Path(args.legacy)
    rusty_path = Path(args.rustypi)
    out_path = Path(args.out)

    legacy, legacy_total, legacy_max_tick = load_counts_by_tick(legacy_path)
    rusty, rusty_total, rusty_max_tick = load_counts_by_tick(rusty_path)
    max_tick = legacy_max_tick if legacy_max_tick > rusty_max_tick else rusty_max_tick

    legacy_counts = total_phase_counts(legacy)
    rusty_counts = total_phase_counts(rusty)
    rusty_pathways = load_pathway_counts(rusty_path)

    html = f"""<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\" />
<title>Choke Trace Report</title>
<style>
body {{ font-family: Segoe UI, Arial, sans-serif; margin: 20px; background: #f7f7fb; color: #111; }}
h1 {{ margin: 0 0 8px 0; }}
.muted {{ color: #555; margin-bottom: 14px; }}
.panel {{ background: #fff; border: 1px solid #ddd; border-radius: 8px; padding: 12px; margin: 12px 0; }}
.legend {{ margin: 8px 0 12px 0; }}
.legend-item {{ display: inline-flex; align-items: center; margin-right: 12px; font-size: 13px; }}
.swatch {{ width: 12px; height: 12px; margin-right: 6px; border-radius: 2px; display: inline-block; }}
svg {{ border: 1px solid #ddd; background: #fff; max-width: 100%; height: auto; }}
table {{ border-collapse: collapse; width: 100%; background: #fff; }}
th, td {{ border: 1px solid #ddd; padding: 6px 8px; text-align: right; }}
th:first-child, td:first-child {{ text-align: left; }}
.code {{ font-family: Consolas, monospace; background: #eee; padding: 2px 4px; border-radius: 4px; }}
</style>
</head>
<body>
  <h1>Choke Trace Report</h1>
  <div class=\"muted\">Legacy: <span class=\"code\">{legacy_path}</span> | RustyPi: <span class=\"code\">{rusty_path}</span></div>
  <div class=\"panel\">
    <b>Row Counts:</b> legacy={legacy_total}, rustypi={rusty_total}, ticks=0..{max_tick}
  </div>
  {render_counts_table(legacy_counts, rusty_counts)}
    {render_pathway_table(rusty_pathways)}
  {render_chart('Legacy Phase Counts Per Tick', legacy, max_tick)}
  {render_chart('RustyPi Phase Counts Per Tick', rusty, max_tick)}
</body>
</html>
"""

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(html, encoding="utf-8")
    print(f"wrote report: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
