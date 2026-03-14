# Project Kernel (Compression-Safe)

Status: canonical, minimal, stable.

This file is the restart anchor when context is lost.

## Non-Negotiable Contract

1. One substance: STE.
2. Primary state: local density rho = STE / area.
3. One interaction law: STE attracts STE.
4. Void core: no carrier transport through core volume.
5. Boundary shelling must emerge from geometry plus conservation.

## Modeling Discipline

1. Deterministic integer transport for core checks.
2. No random initialization for decision runs.
3. No assertion shortcuts for shell persistence.
4. One mechanism change per experiment.
5. Every run must answer one yes/no question.

## Canonical Metrics

1. conservation_error_max
2. core_leak_samples
3. redirection_ratio
4. shell_to_far_peak_ratio
5. shell_emergence_latency

Definitions are in [HYPOTHESIS_LEDGER.md](HYPOTHESIS_LEDGER.md).

## Current Best Reference

Clean-room ontology check output:
- [reports/clean_room/ontology_check.json](../reports/clean_room/ontology_check.json)
- [reports/clean_room/ontology_check.md](../reports/clean_room/ontology_check.md)

## Current Decision State

1. Clean-room single-field check: PASS on conservation, no core leak, and boundary redirection.
2. Production kernel remains exploratory and not yet canonical evidence.

## Do Not Rewrite Rule

Only these sections may change without explicit review:
1. Current Best Reference
2. Current Decision State

All ontology contract lines above are frozen until falsified by a logged experiment.
