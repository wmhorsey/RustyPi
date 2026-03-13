# Falsifiable Laws

This document defines provisional field laws and concrete pass/fail tests.

## Purpose

Convert the STE boundary-flow model into hypotheses that can be disproven by fixed benchmark scenarios.

## Core Variables

- `compression_index`: `max(coherence,0) + max(energy,0)`
- `shell_tension`: `min(max(coherence,0), max(energy,0))`
- `break_pressure`: `max(0, energy - coherence) - shell_tension`

## Law 1: Stable Mid-Band Exists

Statement:
For nested, calm environments, there exists a mid-band where compression is sustained and anchoring is positive without catastrophic dissolution dominance.

Operational test:
- Scenario: `trap`, `target=128`, `generation_depth=2`, `calm_pct=70`
- Pass criteria:
  - `mean_compression_index >= 0.5`
  - `nonzero_compression_ratio >= 0.10`
  - `anchor_retention_gain > 0`

## Law 2: Spikes Ring Down

Statement:
Spike events decay back toward background (finite ring-down), not indefinite growth.

Operational test:
- Scenario: same as Law 1
- Pass criteria:
  - `unresolved_ringdown_ratio <= 0.10`
  - `median_half_life_ticks <= 8`

## Law 3: Catastrophic Collapse Is Rare

Statement:
`dissolution` is a barrier-failure edge case in normal runs, not a dominant steady-state pathway.

Operational test:
- Scenario: same as Law 1
- Pass criteria:
  - `dissolution_ratio <= 0.10`
  - `catastrophic_pathway_ratio <= 0.02`

## Law 4: Boundary-State Occupancy Ordering

Statement:
Under stable nested/calm runs, occupancy should prefer shell-supported regimes over catastrophic release, and persistent void-core signatures should remain rarer than metastable depressed-core shell signatures.

Operational test:
- Scenario: same as Law 1
- Pass criteria:
  - `depressed_shell_occupancy_ratio > persistent_void_occupancy_ratio`
  - `catastrophic_pathway_ratio < depressed_shell_occupancy_ratio`

## Law 5: Frequency-Coupled Reabsorption

Statement:
Higher-frequency swells should show shorter relay lifetime / faster re-equalization than lower-frequency swells under the same boundary conditions.

Operational test:
- Scenario: matched geometry; run low/high frequency channel sweeps
- Pass criteria:
  - `median_relax_ticks_high_freq < median_relax_ticks_low_freq`
  - `high_freq_terminal_capture_ratio >= low_freq_terminal_capture_ratio`

## Benchmark Discipline

1. Keep scenario set fixed for comparability.
2. Change thresholds only with explicit rationale and changelog note.
3. Prefer failing fast over ad-hoc reinterpretation.
4. Treat these laws as provisional until they survive withheld scenarios.
