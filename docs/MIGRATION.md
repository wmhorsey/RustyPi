# Migration Plan (from massEffect)

## Phase 1: Foundation

- Introduce `pi-core` typed phase math.
- Add parity tests against existing float behavior where possible.

## Phase 2: Targeted port

- Port choke/coherence phase handling to `Phase`.
- Port phase-lock/fractal metrics that depend on periodic math.

## Phase 3: Full integration

- Move selected simulation kernels into `pi-sim`.
- Keep viewers in existing repo until interfaces stabilize.

## Phase 4: Repository split

- Initialize new git remote for `pi-unitary`.
- Preserve history via subtree split or fresh history based on preference.
