#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
from collections import defaultdict
from pathlib import Path


def cell_class(pass_count: int) -> str:
    if pass_count >= 7:
        return "pass"
    if pass_count >= 6:
        return "warn"
    return "fail"


def cell_label(pass_count: int, anchor: float, ci: float) -> str:
    return f"{pass_count}/7 | A={anchor:.2f} | CI={ci:.2f}"


def parse_rows(path: Path) -> list[dict[str, str]]:
    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        return list(reader)


def render_table(rows: list[dict[str, str]], channel: str, depth: str, calm: str) -> str:
    filtered = [r for r in rows if r["channel"] == channel and r["depth"] == depth and r["calm_pct"] == calm]
    if not filtered:
        return ""

    targets = sorted({int(r["target"]) for r in filtered})
    by_target = {int(r["target"]): r for r in filtered}

    cells = []
    for t in targets:
        r = by_target[t]
        p = int(r["pass_count"])
        a = float(r["anchor_retention_gain"])
        ci = float(r["mean_compression_index"])
        cls = cell_class(p)
        lbl = cell_label(p, a, ci)
        cells.append(f"<td class='{cls}'><div class='t'>{t}</div><div class='m'>{lbl}</div></td>")

    hdr = "".join(f"<th>{t}</th>" for t in targets)
    row = "".join(cells)
    title = f"channel={channel} depth={depth} calm={calm}"

    return (
        f"<section class='panel'><h3>{title}</h3>"
        f"<table><thead><tr>{hdr}</tr></thead><tbody><tr>{row}</tr></tbody></table>"
        "</section>"
    )


def main() -> int:
    ap = argparse.ArgumentParser(description="Render HTML phase diagram from limit_heatmap.csv")
    ap.add_argument("--in", dest="in_path", required=True, help="Input heatmap CSV")
    ap.add_argument("--out", required=True, help="Output HTML path")
    args = ap.parse_args()

    in_path = Path(args.in_path)
    out_path = Path(args.out)

    rows = parse_rows(in_path)
    if not rows:
        raise SystemExit("No rows found in heatmap CSV")

    channels = sorted({r["channel"] for r in rows})
    depths = sorted({r["depth"] for r in rows}, key=lambda x: int(x))
    calms = sorted({r["calm_pct"] for r in rows}, key=lambda x: int(x))

    sections: list[str] = []
    for ch in channels:
        sections.append(f"<h2>{ch}</h2>")
        for d in depths:
            for c in calms:
                s = render_table(rows, ch, d, c)
                if s:
                    sections.append(s)

    html = f"""<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\" />
<title>Limit Heatmap</title>
<style>
body {{ font-family: Segoe UI, Arial, sans-serif; margin: 20px; background: #f7f7fb; color: #111; }}
h1,h2 {{ margin: 0 0 10px 0; }}
.panel {{ background: #fff; border: 1px solid #ddd; border-radius: 8px; padding: 10px; margin: 10px 0; }}
table {{ border-collapse: collapse; width: 100%; table-layout: fixed; }}
th, td {{ border: 1px solid #ddd; padding: 8px; text-align: center; vertical-align: top; }}
.pass {{ background: #e6f6e9; }}
.warn {{ background: #fff4d6; }}
.fail {{ background: #fde8e8; }}
.t {{ font-weight: 700; margin-bottom: 4px; }}
.m {{ font-size: 12px; color: #333; }}
.legend span {{ display: inline-block; margin-right: 14px; font-size: 13px; }}
.sw {{ width: 12px; height: 12px; display: inline-block; margin-right: 6px; vertical-align: middle; border: 1px solid #bbb; }}
</style>
</head>
<body>
  <h1>Limit Heatmap</h1>
  <div class=\"legend\">
    <span><span class=\"sw\" style=\"background:#e6f6e9\"></span>7/7 pass</span>
    <span><span class=\"sw\" style=\"background:#fff4d6\"></span>6/7 warning</span>
    <span><span class=\"sw\" style=\"background:#fde8e8\"></span>&lt;=5/7 fail</span>
  </div>
  <p>Cell metrics: <b>pass_count</b> | <b>anchor_retention_gain (A)</b> | <b>mean_compression_index (CI)</b></p>
  {''.join(sections)}
</body>
</html>
"""

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(html, encoding="utf-8")
    print(f"wrote heatmap report: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
