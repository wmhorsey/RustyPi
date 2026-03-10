# Geometry-Locked Math Contract

This document defines the non-negotiable numeric rules for `pi-core` and `pi-sim`.

## 1) Core principle: geometry first

All periodic dynamics are represented as geometry on a closed phase manifold (Tau domain), not as free-form raw floats.

- Angles/phases live in Tau-wrapped space.
- Phase distances use shortest-arc metrics.
- Windows/gates are explicit typed domains.

## 2) No-BS numeric policy

"No BS" means:

- No silent domain violations.
- No hidden fallback constants.
- No implicit divide-by-zero behavior.
- No untracked precision loss at update boundaries.

## 3) Zero-safe arithmetic rules

### 3.1 Forbidden behavior

- Direct division where denominator can be zero/near-zero without checks.
- Returning `NaN`/`Inf` to downstream simulation state.
- Using epsilon guards that are unnamed or inconsistent.

### 3.2 Required behavior

- Any risky operation uses explicit safe wrappers.
- Wrappers return either:
  - `Result<T, MathError>` for recoverable failures, or
  - deterministic clamped fallback through named policy functions.
- Policy constants are centralized and named.

Suggested API shape:

```rust
pub enum MathError {
    DivideByZero,
    DomainViolation(&'static str),
}

pub fn safe_div(n: f64, d: f64) -> Result<f64, MathError>;
pub fn checked_sqrt(x: f64) -> Result<f64, MathError>;
```

## 4) Precision and rounding policy

Plain-language note: normal computer decimals (`f64`/`f32`) cannot represent every real value exactly.
So this contract does not promise "zero rounding forever" when floats are used.
It does require deterministic, bounded precision behavior.

### 4.1 Deterministic precision

- Same inputs -> same outputs across runs on the same target.
- Quantization points are explicit (named helpers), never ad-hoc rounding.
- Periodic normalization (`wrap_tau`) is applied at canonical boundaries.

### 4.2 Allowed strategies

- Typed wrappers around `f64` with canonical normalization.
- Optional fixed-point for selected critical accumulators.
- Explicit quantize/dequantize helpers at state boundaries.

### 4.3 Prohibited strategies

- Randomly mixing rounded/unrounded paths.
- Implicit casts that hide precision truncation.
- Silent denormal/overflow propagation into sim state.

## 5) Constant policy

- No unnamed magic numbers in core math.
- Every threshold is named with ontology or geometric meaning.
- Constants are grouped by domain (phase, coherence, saturation, etc.).

## 6) Additive-only kernel mode (preferred for recursion/fractals)

For recursive/fractal kernels, the preferred update path is additive-only in the hot loop.

- Allowed in hot-path updates: `+`, `-`, comparisons, indexing, branch rules.
- Avoided in hot-path updates: `*`, `/`, `%`, floating trig.
- Geometry is represented with integer phase rings (`PhaseTicks`) and lookup/schedule rules.
- Distribution/normalization uses remainder accumulators, not direct division in updates.

This mode is how we enforce "geometry locked, no BS" while preserving determinism.

Mechanical enforcement:

- CI runs `scripts/check_additive_kernel_ops.py`.
- The check fails if designated additive kernel files contain `*`, `/`, or `%` operators.
- Keep the target file list in that script aligned with active additive hot-path modules.

## 7) Invariant test requirements

At minimum, tests must cover:

1. Tau wrapping into `[0, TAU)`.
2. Shortest-arc phase distance symmetry.
3. Gate continuity at center and boundary.
4. No `NaN`/`Inf` outputs under boundary stress cases.
5. Stable behavior under repeated wrap/accumulate cycles.
6. Additive-kernel conservation/monotonicity checks for recursive updates.

## 8) Implementation checklist for new math code

Before merging any numeric change:

1. Is domain encoded in types (not comments)?
2. Are all unsafe operations guarded by explicit policy?
3. Are constants named and documented?
4. Are boundary and periodic invariants tested?
5. Can this path emit `NaN`/`Inf`? If yes, reject.
