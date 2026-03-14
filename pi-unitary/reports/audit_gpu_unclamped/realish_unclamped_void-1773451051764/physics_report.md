# Choke Physics Health Report

- source_csv: reports\audit_gpu_unclamped\realish_unclamped_void-1773451051764\decoded_choke_nodes.csv
- overall_status: FAIL

## What This Means
One or more core invariants failed. Treat this run as suspect until the failed checks are resolved.

## Checks
- PASS: csv not empty (rows=409600)
- PASS: no duplicate (tick,sequence,node) (duplicates=0)
- PASS: constant node count per tick (min=2048, max=2048, ticks=200)
- FAIL: non-negative energy/coherence (neg_energy=0, neg_coherence=246281)
- PASS: phase ids match phase labels (mismatches=0)
- PASS: pathway labels follow phase/energy rules (mismatches=0)
- PASS: pathway ids match pathway labels (mismatches=0)

## Evolution Snapshot
- ticks: 0 -> 4975 (200 snapshots)
- nodes_per_tick: 2048
- active_nodes: 1535 -> 0
- first_tick_phase_counts: {"formation": 1535, "free": 513}
- last_tick_phase_counts: {"free": 2048}

## Totals Across Run
- rows: 409600
- phase_totals: {"dissolution": 68, "formation": 60692, "free": 348840}

## Boundary Profile
- phase_tick_boundary: periodic_mod_4096
- phase_tick_wrap_events: 0
- snapshot_tick_stride: 25
- avg_phase_tick_delta_per_snapshot: 2.6768
- effective_field_rate_per_tick: 0.107073
- inferred_boundary_tension: 9.3394
- configured_boundary_rate: 1/7
- rational_residual_abs_total: 364595
- rational_residual_max_abs: 4
- floor_shares: {"coherence_zero_share": 0.2506, "energy_zero_share": 0.2505, "shell_zero_share": 0.2513, "spin_zero_share": 0.2505}
- interpretation: lower-state channels are strongly pinned to zero when excitation is not sustained; phase progression wraps on a periodic 4096-tick cycle.
