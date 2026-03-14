# Choke Physics Health Report

- source_csv: reports\audit_gpu_rational\rational_1_7-1773434242029\decoded_choke_nodes.csv
- overall_status: PASS

## What This Means
Core ledger invariants hold for this run: accounting is consistent, value domains are valid, and phase/pathway contracts are internally coherent.

## Checks
- PASS: csv not empty (rows=5120)
- PASS: no duplicate (tick,sequence,node) (duplicates=0)
- PASS: constant node count per tick (min=512, max=512, ticks=10)
- PASS: non-negative energy/coherence (neg_energy=0, neg_coherence=0)
- PASS: phase ids match phase labels (mismatches=0)
- PASS: pathway labels follow phase/energy rules (mismatches=0)
- PASS: pathway ids match pathway labels (mismatches=0)

## Evolution Snapshot
- ticks: 0 -> 450 (10 snapshots)
- nodes_per_tick: 512
- active_nodes: 127 -> 12
- first_tick_phase_counts: {"formation": 127, "free": 385}
- last_tick_phase_counts: {"dissolution": 2, "formation": 10, "free": 500}

## Totals Across Run
- rows: 5120
- phase_totals: {"dissolution": 21, "formation": 234, "free": 4865}

## Boundary Profile
- phase_tick_boundary: periodic_mod_4096
- phase_tick_wrap_events: 0
- snapshot_tick_stride: 50
- avg_phase_tick_delta_per_snapshot: 7.1428
- effective_field_rate_per_tick: 0.142856
- inferred_boundary_tension: 7.0001
- configured_boundary_rate: 1/7
- rational_residual_abs_total: 0
- rational_residual_max_abs: 0
- floor_shares: {"coherence_zero_share": 0.9332, "energy_zero_share": 0.8727, "shell_zero_share": 0.9938, "spin_zero_share": 0.9934}
- interpretation: lower-state channels are strongly pinned to zero when excitation is not sustained; phase progression wraps on a periodic 4096-tick cycle.
