# Hypothesis Ledger

Purpose: Keep modeling decisions falsifiable. Every run must answer one yes/no question.

## Ground Contract

1. One substance: STE.
2. Primary state: local density rho = STE / area.
3. One interaction law: STE attracts STE.
4. Void core means no carrier transport through the core volume.
5. Boundary effects must emerge from geometry and conservation, not assertion shortcuts.

## Metrics Dictionary

Use metrics from [reports/clean_room/ontology_check.json](../reports/clean_room/ontology_check.json).

1. conservation_error_max: absolute STE conservation error over run.
2. core_leak_samples: non-zero state samples inside void core.
3. redirection_ratio: redirected_total / blocked_total at void boundary.
4. shell_to_far_peak_ratio: peak boundary density proxy over peak far-field density proxy.
5. shell_emergence_latency: first tick where shell metric exceeds far-field baseline threshold.

## Active Hypotheses

### H1: Geometry-Forced Shelling

Statement:
When inward transport is blocked by a void boundary, tangential redirection alone produces shell intensification.

Pass criteria:
1. conservation_error_max = 0
2. core_leak_samples = 0
3. redirection_ratio >= 0.95
4. shell_to_far_peak_ratio > 1.0

Falsifier:
Any run satisfying 1 to 3 while shell_to_far_peak_ratio <= 1.0.

Status:
Current clean-room check supports H1.

### H2: No Hidden State Law Needed

Statement:
Independent state assertions are unnecessary if transport and boundary geometry are conservative and explicit.

Pass criteria:
1. No special wound/shell assertion branch in update law.
2. H1 pass criteria still hold.

Falsifier:
Removing assertion shortcuts causes failure of H1 while conservation and no-core-leak remain valid.

Status:
In progress in production kernel; clean-room reference passes.

### H3: Compression Is Trapping Plus Motion

Statement:
Local compression is increased density due to transport convergence from motion, trapping, or both.

Pass criteria:
1. abs_field_growth_ratio > 1.0 in convergent runs.
2. Growth is localized at boundary-constrained regions, not uniform across domain.

Falsifier:
Uniform global growth without boundary localization under constrained geometry.

Status:
Partially supported; needs dedicated sweep with radial-bin diagnostics.

## Competing Explanations

### C1: Multi-channel heuristic artifact

Claim:
Observed shelling is due to heuristic channel coupling rather than geometry.

Discriminator test:
Run clean-room single-field model versus production model under identical boundary setup. If only production shells, C1 gains support.

### C2: Numerical artifact from quantization

Claim:
Shelling is mostly integer stepping artifact.

Discriminator test:
Repeat clean-room with move_div sweep and larger grid. If shelling vanishes under refinement while conservation stays exact, C2 gains support.

## Canonical Run Set

1. Control: no void boundary.
2. Void: blocked inward transport with redirection.
3. Void plus high-drive: same geometry under increased flow load.

Record all three with the same tick budget and grid.

## Decision Rule Per Run

Before run:
1. Write one question.
2. Set expected direction for each metric.

After run:
1. Mark PASS or FAIL.
2. If FAIL, update only one mechanism before next run.
3. No multi-change runs.

## Immediate Next Experiments

1. Quantization sensitivity sweep on clean-room model:
- vary move_div across 16, 24, 32, 48
- test stability of shell_to_far_peak_ratio and redirection_ratio

2. Radial-bin localization report:
- replace sampled far nodes with annular bins
- verify compression localization at boundary shell

3. Production parity check:
- run same canonical set in production kernel
- compare sign and ordering of metrics with clean-room reference
