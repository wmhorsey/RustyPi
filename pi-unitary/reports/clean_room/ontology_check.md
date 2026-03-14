# Clean-Room Ontology Check

Model contract:
- single substance: STE
- primary state: density
- deterministic integer transport
- geometry-forced boundary redirection at void interface

Run config:
- ticks: 800
- grid: 129x129
- core radius: 12
- move divisor: 24

## Control (no void)
- total_ste_initial: 4,546,860
- total_ste_final: 4,546,860
- conservation_error_max: 0
- shell_to_far_peak_ratio: 2.3064
- shell_emergence_latency: 0

## Void (blocked inward transport)
- total_ste_initial: 4,371,636
- total_ste_final: 4,371,636
- conservation_error_max: 0
- core_leak_samples: 0
- blocked_total: 13,305,318
- redirected_total: 13,305,318
- redirection_ratio: 1.0
- shell_to_far_peak_ratio: 22.2956
- shell_emergence_latency: 0

## Acceptance
- conservation: PASS
- no_core_leak: PASS
- redirection_effective: PASS
- shell_emerges: PASS
- overall_pass: PASS

Source JSON:
- reports/clean_room/ontology_check.json
