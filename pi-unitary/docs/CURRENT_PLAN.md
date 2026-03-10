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

- `Free` STE is a pool/reservoir state (ocean-like background capacity), not a lifecycle stage in itself.
- Choke onset is resistance-to-flow emergence inside that pool; shell entrapment follows as resistance stabilizes.
- `Dissolution` is constrained STE being released back into `free`.
- Working backward from `Dissolution` is choke assembly: `free -> formation -> lift-off -> coherence -> drift`, where the choke becomes trapped inside an STE shell.
- Calibration changes must preserve this directionality: no direct `free -> dissolution` path and no `coherence/drift` skip when constructing trapped-shell states.

Shell-boundary rule (authoritative):

- `free` is not a timed next-phase; it means shell non-existence.
- Dissolution completion is governed by shell tension, not a hold timer.
- Current additive shell-tension definition in kernel:
	- `shell_tension = min(max(coherence, 0), max(energy, 0))`
- Interpretation:
	- both channels must remain present for a meaningful shell boundary.
	- if either channel is spent, tension collapses and dissolution can release to `free`.

Quantization/boundary rule (authoritative):

- Free-flow STE pool is pre-quantized continuum; there are no persistent levels before resistance forms.
- Quantization appears at choke boundaries, where flow resistance creates a constrained shell state.
- A choke core is a lower-attraction depression relative to surrounding STE, which circulates around and maintains the boundary.
- Dissolution onset is asymmetric boundary failure: when outside energy contrast becomes lopsided, shell support migrates to one side and a channel opens to the free pool.

Discontinuity interpretation (authoritative):

- Quantization lifecycle is modeled as STE discontinuity formation and decay.
- A hot-spot discontinuity is a local energy peak that behaves like a virtual particle proxy until it radiates away.
- A void-like discontinuity is a lower-attraction core protected by surrounding shell flow; shell structure can mimic a protective boundary until contrast collapses.
- Both are boundary states of the same STE medium and must terminate by release/radiation back into free pool continuity.

Interaction quantization hypothesis (authoritative):

- The key balance is not "attraction wave alone," but how local structure forces quantization during interaction.
- Particle-class behavior is modeled as response-to-structure channels:
	- electron-like channel: boundary-trapping dominant response.
	- photon-like channel: boundary-transit/radiative-release dominant response.
- Both channels interact with the same local STE discontinuity geometry and differ by how strongly they retain vs shed shell tension.
- Calibration objective is to recover these contrasting interaction outcomes from one shared shell-tension/break-pressure framework.

Implementation sketch for shell break (next):

- Track boundary asymmetry as an additive contrast between local shell support and local refill pressure.
- Suggested signal:
	- `shell_support = min(coherence, energy)`
	- `refill_pressure = max(0, energy - coherence)`
	- `break_pressure = refill_pressure - shell_support`
- Rule:
	- if `break_pressure` stays positive long enough, move `drift -> dissolution` and accelerate release to `free`.

Latest measured compare (`32 nodes x 1024 ticks`):

- Trap-biased channel:
	- `phase match`: `0.699`
	- `bucket match (free/active)`: `0.702`
	- `coherence MAE`: `0.279293`
	- `energy MAE`: `1.034628`
	- `temporal normalized l1`: `0.546078`
- Radiative-biased channel:
	- `phase match`: `0.705`
	- `bucket match (free/active)`: `0.707`
	- `coherence MAE`: `0.262936`
	- `energy MAE`: `1.040241`
	- `temporal normalized l1`: `0.558333`

RustyPi phase occupancy now includes explicit late states (channel dependent).

Current dominant mismatches (legacy -> RustyPi):

- `dissolution -> free` remains the top structural mismatch in both channels.
- `formation -> free` and `liftoff -> free` remain secondary mismatches.

Immediate tuning direction:

1. Implement additive shell asymmetry / break-pressure signal in `choke_additive.rs` for `drift -> dissolution` gating.
2. Tune release rate with break-pressure so `dissolution -> free` is governed by contrast loss, not fixed dwell.
3. Add response-channel parameters (trap-biased vs radiative-biased) on top of the same boundary model, then compare transition fingerprints.
4. Keep comparator directionality checks as hard gates and use transition-matrix deltas as primary parity metric.
