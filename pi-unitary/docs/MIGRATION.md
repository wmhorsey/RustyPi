# Migration Plan (from massEffect)

## Phase 0: Terminology lock

- Normalize ontology wording across docs before deeper kernel moves.
- Adopt canonical boundary-state names:
	- transient spike state
	- depressed-core shell state
	- persistent void-core shell state
- Carry the wave-frequency coupling rule into planning and test docs.

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
- Preserve ontology contract text in top-level docs during split.
