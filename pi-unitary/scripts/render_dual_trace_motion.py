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


def load_trace(path: Path) -> dict:
    by_tick: dict[int, list[dict]] = {}
    max_tick = 0
    max_node = 0
    max_ci = 1.0

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
        by_node = {r["node"]: r for r in rows}

        nodes = []
        phase_counts = {p: 0 for p in PHASES}
        pathway_counts: dict[str, int] = {}

        for node in range(node_count):
            r = by_node.get(node)
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
            nodes.append([phase_idx, ci, pt, pw])
            phase_counts[phase] = phase_counts.get(phase, 0) + 1
            if pw:
                pathway_counts[pw] = pathway_counts.get(pw, 0) + 1

        frames.append(
            {
                "nodes": nodes,
                "phase_counts": [phase_counts[p] for p in PHASES],
                "pathway_counts": pathway_counts,
            }
        )

    return {
        "frames": frames,
        "node_count": node_count,
        "max_ci_milli": int(round(max_ci * 1000.0)),
    }


def render_html(legacy: dict, rusty: dict, legacy_path: Path, rusty_path: Path) -> str:
    payload = json.dumps(
        {
            "legacy": legacy,
            "rusty": rusty,
            "phases": PHASES,
            "phase_colors": PHASE_COLORS,
        },
        separators=(",", ":"),
    )

    return f"""<!doctype html>
<html lang=\"en\">
<head>
<meta charset=\"utf-8\" />
<title>Dual Trace Motion</title>
<style>
body {{ font-family: Segoe UI, Arial, sans-serif; margin: 0; background: #0f1117; color: #e6e6e6; }}
.wrap {{ padding: 12px; max-width: 1400px; margin: 0 auto; }}
h1 {{ margin: 0 0 8px 0; font-size: 20px; }}
.muted {{ color: #9aa4b2; font-size: 12px; margin-bottom: 10px; }}
.panel {{ background: #171b24; border: 1px solid #283041; border-radius: 8px; padding: 10px; margin-bottom: 10px; }}
.controls {{ display: flex; gap: 8px; align-items: center; flex-wrap: wrap; }}
button, select {{ background: #263042; color: #e6e6e6; border: 1px solid #3b4a63; padding: 6px 10px; border-radius: 6px; }}
input[type=range] {{ width: 360px; }}
.grid {{ display: grid; grid-template-columns: 1fr 1fr; gap: 10px; }}
canvas {{ width: 100%; height: auto; border: 1px solid #2a3448; border-radius: 6px; background: #0b0d12; }}
.title {{ margin: 0 0 6px 0; font-size: 14px; color: #cfd6e1; }}
.stats {{ font-size: 12px; color: #cfd6e1; white-space: pre-wrap; margin-top: 6px; }}
.legend {{ display: flex; gap: 12px; flex-wrap: wrap; margin-top: 8px; }}
.legend-item {{ font-size: 12px; display: inline-flex; align-items: center; gap: 6px; }}
.sw {{ width: 10px; height: 10px; border-radius: 2px; display: inline-block; }}
</style>
</head>
<body>
<div class=\"wrap\">
  <h1>Dual Trace Motion Viewer</h1>
  <div class=\"muted\">Legacy: {legacy_path} | RustyPi: {rusty_path}</div>

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
  </div>

  <div class=\"grid\">
    <div class=\"panel\">
      <div class=\"title\">Legacy</div>
      <canvas id=\"legacyCanvas\" width=\"660\" height=\"520\"></canvas>
      <div class=\"stats\" id=\"legacyStats\"></div>
    </div>
    <div class=\"panel\">
      <div class=\"title\">RustyPi</div>
      <canvas id=\"rustyCanvas\" width=\"660\" height=\"520\"></canvas>
      <div class=\"stats\" id=\"rustyStats\"></div>
    </div>
  </div>
</div>

<script>
const DATA = {payload};
const PHASES = DATA.phases;
const COLORS = DATA.phase_colors;
const legacy = DATA.legacy;
const rusty = DATA.rusty;

const maxFrames = Math.max(legacy.frames.length, rusty.frames.length);
const maxCi = Math.max(1, legacy.max_ci_milli, rusty.max_ci_milli);

const slider = document.getElementById('tickSlider');
const tickLabel = document.getElementById('tickLabel');
const playBtn = document.getElementById('playBtn');
const speedSel = document.getElementById('speedSel');
const legend = document.getElementById('legend');
slider.max = String(Math.max(0, maxFrames - 1));

for (const p of PHASES) {{
  const el = document.createElement('span');
  el.className = 'legend-item';
  el.innerHTML = `<span class=\"sw\" style=\"background:${{COLORS[p] || '#999'}}\"></span>${{p}}`;
  legend.appendChild(el);
}}

const legacyCanvas = document.getElementById('legacyCanvas');
const rustyCanvas = document.getElementById('rustyCanvas');
const legacyCtx = legacyCanvas.getContext('2d');
const rustyCtx = rustyCanvas.getContext('2d');
const legacyStats = document.getElementById('legacyStats');
const rustyStats = document.getElementById('rustyStats');

function drawOne(ctx, canvas, frame, nodeCount, statsEl) {{
  const w = canvas.width;
  const h = canvas.height;
  ctx.clearRect(0, 0, w, h);

  const cx = 220;
  const cy = h / 2;
  const baseR = 80;

  ctx.strokeStyle = '#263347';
  for (let i = 0; i < 4; i++) {{
    ctx.beginPath();
    ctx.arc(cx, cy, baseR + i * 45, 0, Math.PI * 2);
    ctx.stroke();
  }}

  const nodes = frame ? frame.nodes : [];
  const tau = 4096.0;
  for (let i = 0; i < nodes.length; i++) {{
    const n = nodes[i];
    const phaseIdx = n[0];
    const ciMilli = n[1];
    const phaseTick = n[2];
    const phase = PHASES[phaseIdx] || 'free';

    const angle = (phaseTick / tau) * Math.PI * 2.0;
    const r = baseR + (ciMilli / maxCi) * 170.0;
    const x = cx + Math.cos(angle) * r;
    const y = cy + Math.sin(angle) * r;
    const size = 2.0 + (ciMilli / maxCi) * 3.0;

    ctx.beginPath();
    ctx.fillStyle = COLORS[phase] || '#999';
    ctx.arc(x, y, size, 0, Math.PI * 2);
    ctx.fill();
  }}

  // phase bars
  const bx = 430;
  const by = 60;
  const bw = 190;
  const bh = 18;
  const counts = frame ? frame.phase_counts : [nodeCount, 0, 0, 0, 0, 0];

  ctx.font = '12px Segoe UI';
  for (let i = 0; i < PHASES.length; i++) {{
    const p = PHASES[i];
    const c = counts[i] || 0;
    const y = by + i * 30;

    ctx.fillStyle = '#9aa4b2';
    ctx.fillText(`${{p}} (${{c}})`, bx, y - 4);
    ctx.fillStyle = '#1f2633';
    ctx.fillRect(bx, y, bw, bh);
    ctx.fillStyle = COLORS[p] || '#777';
    ctx.fillRect(bx, y, Math.round((c / Math.max(1, nodeCount)) * bw), bh);
  }}

  const pcounts = (frame && frame.pathway_counts) ? frame.pathway_counts : {{}};
  const topPath = Object.entries(pcounts).sort((a,b) => b[1]-a[1]).slice(0,3).map(([k,v]) => `${{k}}=${{v}}`).join(' | ');
  statsEl.textContent = `node_count=${{nodeCount}}\npathways: ${{topPath || 'n/a'}}`;
}}

let tick = 0;
let playing = false;
let handle = null;

function draw(t) {{
  const lf = legacy.frames[t] || null;
  const rf = rusty.frames[t] || null;
  drawOne(legacyCtx, legacyCanvas, lf, legacy.node_count, legacyStats);
  drawOne(rustyCtx, rustyCanvas, rf, rusty.node_count, rustyStats);
  tickLabel.textContent = `tick=${{t}} / ${{maxFrames - 1}}`;
}}

function step() {{
  if (!playing) return;
  const speed = Number(speedSel.value || '1');
  tick += speed;
  if (tick >= maxFrames) {{
    tick = maxFrames - 1;
    playing = false;
    playBtn.textContent = 'Play';
  }}
  slider.value = String(tick);
  draw(tick);
  if (playing) handle = setTimeout(step, 80);
}}

slider.addEventListener('input', () => {{
  tick = Number(slider.value);
  draw(tick);
}});

playBtn.addEventListener('click', () => {{
  playing = !playing;
  playBtn.textContent = playing ? 'Pause' : 'Play';
  if (playing) step();
  else if (handle) clearTimeout(handle);
}});

draw(0);
</script>
</body>
</html>
"""


def main() -> int:
    ap = argparse.ArgumentParser(description="Render synced dual motion viewer (legacy vs RustyPi)")
    ap.add_argument("--legacy", required=True, help="Legacy trace CSV")
    ap.add_argument("--rustypi", required=True, help="RustyPi trace CSV")
    ap.add_argument("--out", required=True, help="Output HTML")
    args = ap.parse_args()

    legacy_path = Path(args.legacy)
    rusty_path = Path(args.rustypi)
    out_path = Path(args.out)

    legacy = load_trace(legacy_path)
    rusty = load_trace(rusty_path)

    html = render_html(legacy, rusty, legacy_path, rusty_path)
    out_path.parent.mkdir(parents=True, exist_ok=True)
    out_path.write_text(html, encoding="utf-8")
    print(f"wrote dual motion viewer: {out_path}")
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
