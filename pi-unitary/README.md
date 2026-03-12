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
