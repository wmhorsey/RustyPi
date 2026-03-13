#!/usr/bin/env python3
from __future__ import annotations

import argparse
import csv
import json
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
    return (s or "").strip().lower()


def normalize_pathway(s: str) -> str:
    return (s or "").strip().lower()


def load_trace(path: Path):
    by_tick: dict[int, list[dict[str, int]]] = {}
    max_tick = 0
    max_node = 0
    max_ci = 1.0
    pathways: set[str] = set()

    with path.open("r", encoding="utf-8", newline="") as f:
        reader = csv.DictReader(f)
        required = {"tick", "node", "coherence", "energy", "phase"}
        missing = required - set(reader.fieldnames or [])
        if missing:
            raise ValueError(f"{path}: missing columns {sorted(missing)}")

        has_phase_tick = "phase_tick" in set(reader.fieldnames or [])
        has_pathway = "pathway" in set(reader.fieldnames or [])

        for rec in reader:
            tick = int(rec["tick"])
            node = int(rec["node"])
            phase = normalize_phase(rec["phase"])
            coherence = float(rec["coherence"])
            energy = float(rec["energy"])
            ci = max(0.0, coherence) + max(0.0, energy)
            if ci > max_ci:
                max_ci = ci

            pt = int(float(rec["phase_tick"])) if has_phase_tick and rec.get("phase_tick") else 0
            pw = normalize_pathway(rec.get("pathway", "")) if has_pathway else ""
            if pw:
                pathways.add(pw)

            if tick not in by_tick:
                by_tick[tick] = []

            by_tick[tick].append(
                {
                    "node": node,
                    "phase": phase,
                    "ci": int(round(ci * 1000.0)),
                    "phase_tick": pt,
                    "pathway": pw,
                }
            )

            if tick > max_tick:
                max_tick = tick
            if node > max_node:
                max_node = node

    node_count = max_node + 1

    frames = []
    for t in range(0, max_tick + 1):
        rows = by_tick.get(t, [])
        rows_by_node = {r["node"]: r for r in rows}

        frame_nodes = []
        phase_counts = {p: 0 for p in PHASES}
        pathway_counts: dict[str, int] = {}

        for node in range(node_count):
            r = rows_by_node.get(node)
            if r is None:
                phase = "free"
                ci = 0
                pt = 0
                pw = ""
            else:
                phase = r["phase"]
                ci = r["ci"]
                pt = r["phase_tick"]
                pw = r["pathway"]

            phase_idx = PHASES.index(phase) if phase in PHASES else 0
            frame_nodes.append([phase_idx, ci, pt, pw])
            phase_counts[phase] = phase_counts.get(phase, 0) + 1
            if pw:
                pathway_counts[pw] = pathway_counts.get(pw, 0) + 1

        frames.append(
            {
                "nodes": frame_nodes,
                "phase_counts": [phase_counts[p] for p in PHASES],
                "pathway_counts": pathway_counts,
            }
        )

    return {
        "frames": frames,
        "phases": PHASES,
        "phase_colors": PHASE_COLORS,
        "max_ci_milli": int(round(max_ci * 1000.0)),
        "node_count": node_count,
        "pathways": sorted(pathways),
    }


def render_html(data: dict, source_path: Path) -> str:
    payload = json.dumps(data, separators=(",", ":"))

    return f"""<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\" />
<title>Trace Motion Viewer</title>
<style>
body {{ font-family: Segoe UI, Arial, sans-serif; margin: 0; background: #0f1117; color: #e6e6e6; }}
.wrap {{ padding: 12px; max-width: 1200px; margin: 0 auto; }}
h1 {{ margin: 0 0 8px 0; font-size: 20px; }}
.muted {{ color: #9aa4b2; font-size: 12px; margin-bottom: 10px; }}
.panel {{ background: #171b24; border: 1px solid #283041; border-radius: 8px; padding: 10px; margin-bottom: 10px; }}
.controls {{ display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }}
button, select {{ background: #263042; color: #e6e6e6; border: 1px solid #3b4a63; padding: 6px 10px; border-radius: 6px; }}
input[type=range] {{ width: 320px; }}
canvas {{ width: 100%; height: auto; border: 1px solid #2a3448; border-radius: 6px; background: #0b0d12; }}
.legend {{ display: flex; gap: 12px; flex-wrap: wrap; margin-top: 8px; }}
.legend-item {{ font-size: 12px; display: inline-flex; align-items: center; gap: 6px; }}
.sw {{ width: 10px; height: 10px; border-radius: 2px; display: inline-block; }}
.stats {{ font-size: 12px; color: #cfd6e1; margin-top: 6px; white-space: pre-wrap; }}
</style>
</head>
<body>
<div class=\"wrap\">
  <h1>Trace Motion Viewer</h1>
  <div class=\"muted\">Source: {source_path}</div>

  <div class=\"panel\">
    <div class=\"controls\">
      <button id=\"playBtn\">Play</button>
      <label>Tick <input type=\"range\" id=\"tickSlider\" min=\"0\" max=\"0\" value=\"0\" /></label>
      <span id=\"tickLabel\">tick=0</span>
      <label>Speed
        <select id=\"speedSel\">
          <option value=\"1\">1x</option>
          <option value=\"2\">2x</option>
          <option value=\"4\">4x</option>
          <option value=\"8\">8x</option>
        </select>
      </label>
    </div>
    <div class=\"legend\" id=\"legend\"></div>
    <div class=\"stats\" id=\"stats\"></div>
  </div>

  <div class=\"panel\">
    <canvas id=\"fieldCanvas\" width=\"1140\" height=\"620\"></canvas>
  </div>
</div>

<script>
const DATA = {payload};
const PHASES = DATA.phases;
const COLORS = DATA.phase_colors;
const frames = DATA.frames;
const maxCi = Math.max(1, DATA.max_ci_milli);

const canvas = document.getElementById('fieldCanvas');
const ctx = canvas.getContext('2d');
const slider = document.getElementById('tickSlider');
const tickLabel = document.getElementById('tickLabel');
const playBtn = document.getElementById('playBtn');
const speedSel = document.getElementById('speedSel');
const legend = document.getElementById('legend');
const stats = document.getElementById('stats');

slider.max = String(Math.max(0, frames.length - 1));

for (const p of PHASES) {{
  const el = document.createElement('span');
  el.className = 'legend-item';
  el.innerHTML = `<span class=\"sw\" style=\"background:${{COLORS[p] || '#999'}}\"></span>${{p}}`;
  legend.appendChild(el);
}}

let tick = 0;
let playing = false;
let handle = null;

function drawFrame(t) {{
  const f = frames[t];
  if (!f) return;

  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  // Split layout: left orbital field, right phase bars
  const cx = 360;
  const cy = h / 2;
  const baseR = 160;

  // Field rings
  ctx.strokeStyle = '#263347';
  for (let i = 0; i < 4; i++) {{
    ctx.beginPath();
    ctx.arc(cx, cy, baseR + i * 65, 0, Math.PI * 2);
    ctx.stroke();
  }}

  // Nodes as moving particles
  const tau = 4096.0;
  const nodes = f.nodes;
  for (let i = 0; i < nodes.length; i++) {{
    const n = nodes[i];
    const phaseIdx = n[0];
    const ciMilli = n[1];
    const phaseTick = n[2];

    const phase = PHASES[phaseIdx] || 'free';
    const color = COLORS[phase] || '#9aa4b2';

    const angle = (phaseTick / tau) * Math.PI * 2.0;
    const r = baseR + (ciMilli / maxCi) * 220.0;
    const x = cx + Math.cos(angle) * r;
    const y = cy + Math.sin(angle) * r;
    const size = 2.0 + (ciMilli / maxCi) * 3.5;

    ctx.beginPath();
    ctx.fillStyle = color;
    ctx.arc(x, y, size, 0, Math.PI * 2);
    ctx.fill();
  }}

  // Right-side phase bars
  const bx = 760;
  const by = 80;
  const bw = 320;
  const bh = 26;
  const maxCount = nodes.length;

  ctx.font = '13px Segoe UI';
  for (let i = 0; i < PHASES.length; i++) {{
    const p = PHASES[i];
    const count = f.phase_counts[i] || 0;
    const y = by + i * 40;

    ctx.fillStyle = '#9aa4b2';
    ctx.fillText(`${{p}} (${{count}})`, bx, y - 6);

    ctx.fillStyle = '#1f2633';
    ctx.fillRect(bx, y, bw, bh);

    ctx.fillStyle = COLORS[p] || '#777';
    const fillW = Math.round((count / Math.max(1, maxCount)) * bw);
    ctx.fillRect(bx, y, fillW, bh);
  }}

  tickLabel.textContent = `tick=${{t}} / ${{frames.length - 1}}`;

  const topPathways = Object.entries(f.pathway_counts || {{}})
    .sort((a, b) => b[1] - a[1])
    .slice(0, 4)
    .map(([k, v]) => `${{k}}=${{v}}`)
    .join(' | ');

  stats.textContent =
    `node_count=${{nodes.length}}\n` +
    `dominant pathways: ${{topPathways || 'n/a'}}`;
}}

function step() {{
  if (!playing) return;
  const speed = Number(speedSel.value || '1');
  tick += speed;
  if (tick >= frames.length) {{
    tick = frames.length - 1;
    playing = false;
    playBtn.textContent = 'Play';
  }}
  slider.value = String(tick);
  drawFrame(tick);
  if (playing) {{
    handle = setTimeout(step, 80);
  }}
}}

slider.addEventListener('input', () => {{
  tick = Number(slider.value);
  drawFrame(tick);
}});

playBtn.addEventListener('click', () => {{
  playing = !playing;
  playBtn.textContent = playing ? 'Pause' : 'Play';
  if (playing) step();
  else if (handle) clearTimeout(handle);
}});

drawFrame(0);
</script>
</body>
</html>
"""


def main() -> int:
    ap = argparse.ArgumentParser(description="Render animated HTML motion view from a trace CSV")
    ap.add_argument("--trace", required=True, help="Input trace CSV")
    ap.add_argument("--out", required=True, help="Output HTML path")
    args = ap.parse_args()

    trace_path = Path(args.trace)
    out_path = Path(args.out)

    data = load_trace(trace_path)
    html = render_html(data, trace_path)

    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(html, encoding="utf-8")
    print(f"wrote motion viewer: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
