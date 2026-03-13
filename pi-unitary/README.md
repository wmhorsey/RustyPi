# pi-unitary

Pi-centric unitary mathematics and simulation tooling for STE-style field dynamics.

## Why this exists

This project isolates a Pi/Tau-first numerical foundation from any specific viewer or host app.
It is intended to become a standalone repository.

## Workspace layout

- `crates/pi-core`: foundational Pi/Tau math types and invariants
- `crates/pi-sim`: simulation building blocks using `pi-core`
- `python/pithon`: analysis and experimentation layer for notebooks/scripts
- `docs/`: architecture, migration notes, and execution plan

## Planning docs

- Current execution plan: `docs/CURRENT_PLAN.md`
- Geometry-locked numeric contract: `docs/GEOMETRY_LOCKED_MATH.md`

## RustyPi / PrimalPi Ontology Contract

This workspace treats boundary conditions as first-class ontology primitives.

- "Shell" means a boundary where attraction frustrates and stress must circulate.
- Mass-like behavior is tracked as three boundary regimes:
	- transient spike (radiative ring-down)
	- depressed-core shell (metastable)
	- void-core shell (persistent, STE-zero core with shell-borne stress)
- Heat/light-like channels are swells in the same STE medium.
- Higher frequency swells couple harder to shell boundaries and are reabsorbed faster.

Authoritative implementation wording lives in `docs/CURRENT_PLAN.md`.

## Quick start (Rust)

```bash
cargo check --workspace
cargo test --workspace
```

### Additive lock check

```bash
pwsh ./scripts/check_additive.ps1
```

Fast mode (skip tests):

```bash
pwsh ./scripts/check_additive.ps1 -NoTests
```

### Deterministic choke trace runner

Emit CSV trace to stdout:

```bash
cargo run -p pi-sim --bin choke_trace -- --ticks 128 --nodes 4 --target 0
```

Write CSV to file:

```bash
cargo run -p pi-sim --bin choke_trace -- --ticks 128 --nodes 4 --target 0 --out trace.csv
```

### Legacy vs RustyPi trace comparison

Generate legacy trace from `massEffect/engine`:

```bash
cd ../engine
cargo run --features native --bin choke_trace -- --ticks 128 --nodes 64 --out legacy_trace.csv
```

Generate RustyPi trace:

```bash
cd ../pi-unitary
cargo run -p pi-sim --bin choke_trace -- --ticks 128 --nodes 64 --target 0 --out rustypi_trace.csv
```

Use explicit response channel profile:

```bash
cargo run -p pi-sim --bin choke_trace -- --ticks 128 --nodes 64 --target 0 --channel trap --out rustypi_trace_trap.csv
cargo run -p pi-sim --bin choke_trace -- --ticks 128 --nodes 64 --target 0 --channel radiative --out rustypi_trace_radiative.csv
```

Model nested/calm environment step-up loading:

```bash
cargo run -p pi-sim --bin choke_trace -- --ticks 1024 --nodes 32 --target 128 --channel trap --generation-depth 2 --calm-pct 70 --out rustypi_trace_nested.csv
```

Compare both traces:

```bash
python scripts/compare_choke_traces.py --legacy ../engine/legacy_trace.csv --rustypi rustypi_trace.csv
```

Render a visual HTML report:

```bash
python scripts/render_choke_report.py --legacy ../engine/legacy_trace.csv --rustypi rustypi_trace.csv --out reports/choke_report.html
```

RustyPi traces now include a `pathway` ledger column (`free_pool`, `depression_consumption`, `choke_shell_structuring`, `radiative_release`, `catastrophic_collapse`) and the HTML report renders pathway totals when present.

### Full-state audit trail capture (segment + manifest)

Record high-throughput full-state payloads into an audit run directory:

```bash
cargo run -p pi-sim --bin gpu_audit_capture -- \
	--ticks 10000 \
	--snapshot-every 1 \
	--state-bytes 67108864 \
	--segment-mb 512 \
	--out-dir reports/audit_runs \
	--run-label rtx2070_fullstate
```

Output layout per run:

- `segment_*.rpa`: binary segments with all payload bytes and per-record headers
- `manifest.jsonl`: append-only audit ledger with tick, sequence, offsets, and BLAKE3 hashes
- `run.json`: run summary (duration, byte count, snapshot count)

Integration note:

- Replace synthetic payload fill in `gpu_audit_capture` with actual GPU ping-pong readback bytes.
- Keep the same writer path to preserve deterministic audit and integrity guarantees.

### Native GPU ping-pong full-state capture (wgpu)

Capture actual GPU-resident ping-pong compute states through readback into the same audit format:

```bash
cargo run -p pi-sim --bin gpu_pingpong_audit -- \
	--ticks 4096 \
	--snapshot-every 1 \
	--nodes 2097152 \
	--target-phase 0 \
	--segment-mb 512 \
	--out-dir reports/audit_runs \
	--run-label rtx2070_pingpong
```

This binary performs native compute updates on GPU buffers and records structured choke node snapshots (32 bytes/node) to `segment_*.rpa` with per-snapshot BLAKE3 hashes in `manifest.jsonl`.

Decode a run into row-wise CSV for downstream analysis:

```bash
cargo run -p pi-sim --bin gpu_audit_decode_choke -- \
	--run-dir reports/audit_runs/rtx2070_pingpong-<timestamp> \
	--out decoded_choke_nodes.csv
```

Run a multi-scale stability sweep and auto-generate summary metrics:

```bash
pwsh ./scripts/gpu_scale_probe.ps1 -Ticks 1024 -SnapshotEvery 16 -TargetPhase 128 -NodeScalesCsv "512,2048,8192"
```

This writes `scale_summary.json` under the selected output directory and prints per-scale invariants (`neg_energy`, `neg_coherence`) and phase-share distributions.

### Real-time GPU simulation watcher

Watch a running GPU simulation evolve in real-time with live phase distribution plots:

```bash
pip install matplotlib
python scripts/watch_gpu_choke.py --csv reports/audit_gpu_scale_probe_smoke3/scale_512-1773430469217/decoded_choke_nodes.csv
```

This opens an animated matplotlib window showing:
- Time series of phase counts over simulation ticks
- Current phase distribution pie chart
- Automatic updates as new decoded data becomes available

For console-only environments, use the text-based watcher:

```bash
python scripts/watch_gpu_choke_console.py --csv reports/audit_gpu_scale_probe_smoke3/scale_512-1773430469217/decoded_choke_nodes.csv
```

This prints live updates to the terminal with phase distributions and progress bars.

The comparator now reports:

- phase/bucket match and MAE metrics
- directionality-contract violations (temporal)
- temporal transition delta score (`l1 delta`, `normalized l1`) and top edge deltas

## Quick start (Python)

```bash
cd python/pithon
python -m venv .venv
. .venv/Scripts/activate
pip install -e .
python -c "import pithon; print(pithon.__version__)"
```

## Current roadmap

1. Lock numeric contract (`geometry-locked`, zero-safe, deterministic precision).
2. Expand `pi-core` with typed windows and safe arithmetic wrappers.
3. Port first simulation kernel (`choke/coherence`) to typed phase math.
4. Encode theory as executable invariants before engine/view integration.
