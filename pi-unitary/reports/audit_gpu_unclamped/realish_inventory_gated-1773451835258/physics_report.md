# Choke Physics Health Report

- source_csv: reports\audit_gpu_unclamped\realish_inventory_gated-1773451835258\decoded_choke_nodes.csv
- overall_status: PASS

## What This Means
Core ledger invariants hold for this run: accounting is consistent, value domains are valid, and phase/pathway contracts are internally coherent.

## Checks
- PASS: csv not empty (rows=163840)
- PASS: no duplicate (tick,sequence,node) (duplicates=0)
- PASS: constant node count per tick (min=2048, max=2048, ticks=80)
- PASS: non-negative energy/coherence (neg_energy=0, neg_coherence=0)
- PASS: phase ids match phase labels (mismatches=0)
- PASS: pathway labels follow phase/energy rules (mismatches=0)
- PASS: pathway ids match pathway labels (mismatches=0)

## Evolution Snapshot
- ticks: 0 -> 1975 (80 snapshots)
- nodes_per_tick: 2048
- active_nodes: 1535 -> 6
- first_tick_phase_counts: {"formation": 1535, "free": 513}
- last_tick_phase_counts: {"formation": 6, "free": 2042}

## Totals Across Run
- rows: 163840
- phase_totals: {"dissolution": 67, "formation": 64579, "free": 99194}

## Boundary Profile
- phase_tick_boundary: periodic_mod_4096
- phase_tick_wrap_events: 0
- snapshot_tick_stride: 25
- avg_phase_tick_delta_per_snapshot: 2.6768
- effective_field_rate_per_tick: 0.107073
- inferred_boundary_tension: 9.3394
- configured_boundary_rate: 1/7
- rational_residual_abs_total: 144739
- rational_residual_max_abs: 4
- floor_shares: {"coherence_zero_share": 0.6058, "energy_zero_share": 0.2505, "shell_zero_share": 0.9995, "spin_zero_share": 0.9995}
- interpretation: lower-state channels are strongly pinned to zero when excitation is not sustained; phase progression wraps on a periodic 4096-tick cycle.
