# Current Plan

## Mission

Build a Pi-centric simulation foundation where geometric/phase behavior is explicit, deterministic, and testable before engine/viewer integration.

## Working order

1. Math foundation (`pi-core`)
2. Simulation kernels (`pi-sim`)
3. Theory as executable invariants (tests/docs)
4. Engine/viewer integration (later)

## Active phase (now)

### Phase A: Geometry-Locked math contract

- Define and enforce typed phase/angle units (`Phase`, `PhaseWindow`, `Turn`).
- Define zero-safe arithmetic policy for all operations that can fail.
- Define deterministic precision policy (no hidden float drift decisions).
- Add invariant/property tests for wrap, periodicity, and gating continuity.

Deliverable:
- `docs/GEOMETRY_LOCKED_MATH.md` is the canonical numeric contract.

### Phase B: Expand `pi-core`

- Add `PhaseWindow` type for explicit bounded phase domains.
- Add safe divide helpers with explicit error paths.
- Add deterministic quantization helpers for state update boundaries.
- Add canonical constants registry (named Tau/Pi-domain constants only).
- Add additive-only primitives (`PhaseTicks`, `RemainderAccumulator`) for recursive/fractal kernels.

Deliverable:
- API + tests in `crates/pi-core` for all above primitives.

### Phase C: Port first kernel to `pi-sim`

- Port choke/coherence transitions using typed phases and windows.
- Add additive-only recursive fractal demo kernel (no `*`/`/` in hot update loop).
- Add additive-only choke lifecycle kernel with typed phases and integer coherence transitions.
- Remove ad-hoc scalar thresholds in favor of named constants.
- Add parity tests against baseline behavior where meaningful.

Deliverable:
- `crates/pi-sim` kernel passes tests with explicit domain guards.

## Definition of done (current phase)

- Every phase-related function in `pi-core` is type-guarded.
- Every potentially undefined arithmetic path has explicit handling.
- Test suite covers periodic wrap and boundary behavior.
- Additive recursive kernel tests pass conservation and distribution checks.
- No unnamed constants in core math code.

## Out of scope (for now)

- New DSL parser/compiler.
- Viewer/UI features.
- Performance optimization beyond deterministic correctness.

## Calibration Status (March 2026)

Lifecycle semantics lock (authoritative):

- `Dissolution` is constrained STE being released back into `free`.
- Working backward from `Dissolution` is choke assembly: `free -> formation -> lift-off -> coherence -> drift`, where the choke becomes trapped inside an STE shell.
- Calibration changes must preserve this directionality: no direct `free -> dissolution` path and no `coherence/drift` skip when constructing trapped-shell states.

Latest measured compare (`32 nodes x 1024 ticks`):

- `phase match`: `0.664`
- `bucket match (free/active)`: `0.682`
- `coherence MAE`: `0.269039`
- `energy MAE`: `1.051450`

RustyPi phase occupancy now includes explicit late states:

- `free: 7570`
- `formation: 447`
- `liftoff: 25`
- `dissolution: 150`

Current dominant mismatches (legacy -> RustyPi):

- `dissolution -> free: 1477`
- `formation -> free: 430`
- `free -> formation: 305`

Immediate tuning direction:

1. Increase promoted-node decay residency in `Dissolution` without allowing `Free -> Dissolution` overshoot.
2. Shift deterministic scenario timing to align active bursts with legacy dissolution windows.
3. Track transition-matrix deltas as the primary acceptance signal, with MAE as secondary.
