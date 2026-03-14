# Choke Physics Health Report

- source_csv: reports\audit_gpu_uniform_probe\center0_in7_drive-1773440354453\decoded_choke_nodes.csv
- overall_status: PASS

## What This Means
Core ledger invariants hold for this run: accounting is consistent, value domains are valid, and phase/pathway contracts are internally coherent.

## Checks
- PASS: csv not empty (rows=40960)
- PASS: no duplicate (tick,sequence,node) (duplicates=0)
- PASS: constant node count per tick (min=1024, max=1024, ticks=40)
- PASS: non-negative energy/coherence (neg_energy=0, neg_coherence=0)
- PASS: phase ids match phase labels (mismatches=0)
- PASS: pathway labels follow phase/energy rules (mismatches=0)
- PASS: pathway ids match pathway labels (mismatches=0)

## Evolution Snapshot
- ticks: 0 -> 1950 (40 snapshots)
- nodes_per_tick: 1024
- active_nodes: 188 -> 0
- first_tick_phase_counts: {"formation": 188, "free": 836}
- last_tick_phase_counts: {"free": 1024}

## Totals Across Run
- rows: 40960
- phase_totals: {"dissolution": 39, "formation": 375, "free": 40546}

## Boundary Profile
- phase_tick_boundary: periodic_mod_4096
- phase_tick_wrap_events: 0
- snapshot_tick_stride: 50
- avg_phase_tick_delta_per_snapshot: 5.3502
- effective_field_rate_per_tick: 0.107003
- inferred_boundary_tension: 9.3455
- configured_boundary_rate: 1/7
- rational_residual_abs_total: 71593
- rational_residual_max_abs: 8
- floor_shares: {"coherence_zero_share": 0.9866, "energy_zero_share": 0.9409, "shell_zero_share": 0.9987, "spin_zero_share": 0.9986}
- interpretation: lower-state channels are strongly pinned to zero when excitation is not sustained; phase progression wraps on a periodic 4096-tick cycle.
