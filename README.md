# pi-unitary

Pi-centric unitary mathematics and simulation tooling for STE-style field dynamics.

## Why this exists

This project isolates a Pi/Tau-first numerical foundation from any specific viewer or host app.
It is intended to become a standalone repository.

## Workspace layout

- `crates/pi-core`: foundational Pi/Tau math types and invariants
- `crates/pi-sim`: simulation building blocks using `pi-core`
- `python/pithon`: analysis and experimentation layer for notebooks/scripts
- `docs/`: architecture and migration notes

## Quick start (Rust)

```bash
cargo check --workspace
cargo test --workspace
```

## Quick start (Python)

```bash
cd python/pithon
python -m venv .venv
. .venv/Scripts/activate
pip install -e .
python -c "import pithon; print(pithon.__version__)"
```

## Initial roadmap

1. Normalize phase/angle handling around Tau (`2*pi`).
2. Replace ad-hoc constants with named Pi-domain constants.
3. Port choke/coherence logic to typed phase units.
4. Add invariant/property tests for periodic behavior.
