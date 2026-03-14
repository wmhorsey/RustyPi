# Choke Physics Health Report

- source_csv: reports\audit_gpu_verify_harness\bnd0to7_d7-1773439578693\decoded_choke_nodes.csv
- overall_status: PASS

## What This Means
Core ledger invariants hold for this run: accounting is consistent, value domains are valid, and phase/pathway contracts are internally coherent.

## Checks
- PASS: csv not empty (rows=8192)
- PASS: no duplicate (tick,sequence,node) (duplicates=0)
- PASS: constant node count per tick (min=512, max=512, ticks=16)
- PASS: non-negative energy/coherence (neg_energy=0, neg_coherence=0)
- PASS: phase ids match phase labels (mismatches=0)
- PASS: pathway labels follow phase/energy rules (mismatches=0)
- PASS: pathway ids match pathway labels (mismatches=0)

## Evolution Snapshot
- ticks: 0 -> 240 (16 snapshots)
- nodes_per_tick: 512
- active_nodes: 127 -> 17
- first_tick_phase_counts: {"formation": 127, "free": 385}
- last_tick_phase_counts: {"dissolution": 4, "formation": 13, "free": 495}

## Totals Across Run
- rows: 8192
- phase_totals: {"dissolution": 38, "formation": 307, "free": 7847}

## Boundary Profile
- phase_tick_boundary: periodic_mod_4096
- phase_tick_wrap_events: 0
- snapshot_tick_stride: 16
- avg_phase_tick_delta_per_snapshot: 2.2857
- effective_field_rate_per_tick: 0.142855
- inferred_boundary_tension: 7.0001
- configured_boundary_rate: 1/7
- rational_residual_abs_total: 0
- rational_residual_max_abs: 0
- floor_shares: {"coherence_zero_share": 0.9489, "energy_zero_share": 0.8895, "shell_zero_share": 0.9939, "spin_zero_share": 0.9937}
- interpretation: lower-state channels are strongly pinned to zero when excitation is not sustained; phase progression wraps on a periodic 4096-tick cycle.
