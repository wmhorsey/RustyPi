# Architecture

## Design goals

- Pi/Tau-centric numeric primitives
- Unit-safe APIs to reduce hidden magic numbers
- Separation between numeric core and simulation policy
- Reproducible analysis via Python tooling

## Layers

1. `pi-core`
- `Phase` (tau-wrapped angle)
- Pi/Tau constants and transforms
- deterministic helpers (`sin`, `cos`, interpolation, wrap)

2. `pi-sim`
- State transitions using `Phase` + unitless scalars
- No rendering, no browser-specific assumptions

3. `pithon`
- Analysis utilities mirroring core formulas
- Fast exploratory scripts/notebooks

## Non-goals (initial)

- Full DSL parser/compiler
- Viewer/UI coupling
- Engine-specific compatibility shims
