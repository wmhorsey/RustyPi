# MassEffect Plan

## Purpose
This file is the persistent project compass for the STE model. It exists so context compression or session resets do not lose direction.

## North Star
Build a scale-invariant, collapse-first STE simulation where:
- substrate flow is continuous (not quantized),
- mass emerges as resistance to flow,
- large structures interact through pre-contact saturation fronts,
- behavior is explainable with derived relations rather than magic constants.

## Non-Negotiables
- No unexplained magic numbers in governing physics.
- Constants allowed only for:
  - numerical safety (`eps`, `void_threshold`),
  - unit selection (`parcel_radius_scale`, `dt` as integration resolution),
  - documented calibrated closure terms.
- Same physics stack across scales (proton, nebula, larger structures).
- Every claim must have diagnostics, not just visuals.

## Ontology Guardrails (Hard Constraint)
- Do NOT import standard-physics assumptions as hidden foundations.
- Standard physics terms are allowed only as observational labels or comparison metrics.
- The governing equations must remain STE-native:
  - substrate flow first,
  - collapse/resistance emergence,
  - structure from local relational state.
- Any new term must answer in code comments:
  - "What STE-native mechanism does this represent?"
  - "Why is this not a borrowed assumption?"
- If a proposal cannot be explained in ontology-native terms, do not merge it.

## Current State (As Of 2026-03-05)
Implemented:
- all-pairs field interactions (bond graph removed),
- overlap-capped attraction (prevents singular dot collapse),
- relative choke formation/lifecycle (absolute thresholds removed),
- emergent wave propagation model (time-budget, density-dependent speed),
- spin evolution via viscous torque (no unconditional spin decay),
- close-range spin-coupled parcel interaction,
- new saturation/pressure closure state:
  - saturation `S = C / <C_local>`
  - permeability `K = 1 / (1 + S)`
  - pressure potential `P = C * (1 - K)`
- pre-contact pressure force channel in `apply_forces`,
- proton + nebula views both show saturation/pressure diagnostics (`sat>1`, `fronts`, `Savg`) and pressure halos.
- diagnostics ledger in engine (`diagnostics.rs`) with conservation and force residuals,
- timestep convergence integration test (`tests/convergence.rs`),
- baseline scale-invariance integration test (`tests/scale_invariance.rs`),
- compound coexistence diagnostics in HUD (`pairs`, `dwell`, `pot`, `yield`),
- chirality diagnostics in HUD (`chi`, `zc`, `lock`).

Known caveats:
- pressure channel is early-stage and intentionally conservative (gradient-driven only) to preserve baseline attraction tests,
- diagnostics framework is still ad hoc in HUD and tests; no unified benchmark runner yet,
- centroid recycle can bias long-run morphology in visual modes.

## Immediate Objective
Move from "looks plausible" to "quantitatively defensible" via a diagnostics-first workflow.

## Phase Plan

### Phase 1: Diagnostics Backbone (Priority)
Goal: make every major statement falsifiable.
- Add conservation ledger per tick:
  - total STE,
  - linear momentum,
  - angular momentum,
  - kinetic + shell/choke energy proxies.
- Add action-reaction residual checks on force pairs.
- Add timestep convergence harness (`dt`, `dt/2`, `dt/4`).
- Emit machine-readable run artifacts (CSV/JSON).

Exit criteria:
- drift and residuals are measurable and stable,
- convergence plots are reproducible by script.

### Phase 2: Scale-Invariance Validation
Goal: prove same law stack across scales.
- Define dimensionless initialization template.
- Run proton/nebula/larger-scale comparisons.
- Compare normalized observables:
  - `g(r/r0)`,
  - structure/saturation histograms,
  - choke density and phase fractions,
  - `Savg` and fronts density.

Exit criteria:
- normalized curves collapse within tolerance across scales.

### Phase 3: Thermodynamic Closure Hardening
Goal: make saturation-pressure channel academically robust.
- Empirically fit `P(S)` from runs and report residuals.
- Test alternate derived closures (same variables, no hard thresholds).
- Separate force subchannels in diagnostics:
  - attraction,
  - pressure,
  - spin coupling.

Exit criteria:
- closure form is justified by data, not appearance.

### Phase 4: Publication-Grade Evidence Pack
Goal: withstand skeptical review.
- Produce automated report with:
  - invariants drift,
  - convergence,
  - phase map,
  - ablation matrix,
  - scale-collapse figures.
- Keep reproducible seeds and scripts committed.

Exit criteria:
- third party can rerun and reproduce headline plots.

## Change Protocol (To Avoid Regressions)
For each physics change:
1. State hypothesis.
2. Identify invariant risk.
3. Implement smallest change.
4. Run diagnostics suite.
5. Record pass/fail and next action in commit notes.

## Open Questions
- Exact long-range form of pressure kernel and interaction range scaling.
- Best energy bookkeeping for shell/choke state to close the thermodynamic loop.
- Boundary conditions for science runs (reduce visual-mode recycle bias).

## Short-Term Next Actions
1. Add chirality oscillator diagnostics test/report (`chi`, zero-crossings, lock duration).
2. Add persisted diagnostics artifact output (CSV/JSON) for replay + analysis.
3. Add compound coexistence report (pairs/dwell/potential/yield over time).
4. Build first automated baseline report from diagnostics tests.

## Scope Recovery Checklist
If context is compressed, resume in this order:
1. Re-read this file.
2. Re-assert Ontology Guardrails before making any physics edits.
3. Confirm current physics stack still matches "Current State" above.
4. Run diagnostics baseline.
5. Continue from "Short-Term Next Actions" item 1.
