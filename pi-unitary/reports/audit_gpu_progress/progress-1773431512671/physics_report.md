# Choke Physics Health Report

- source_csv: reports\audit_gpu_progress\progress-1773431512671\decoded_choke_nodes.csv
- overall_status: PASS

## What This Means
Core ledger invariants hold for this run: accounting is consistent, value domains are valid, and phase/pathway contracts are internally coherent.

## Checks
- PASS: csv not empty (rows=6144)
- PASS: no duplicate (tick,sequence,node) (duplicates=0)
- PASS: constant node count per tick (min=512, max=512, ticks=12)
- PASS: non-negative energy/coherence (neg_energy=0, neg_coherence=0)
- PASS: phase ids match phase labels (mismatches=0)
- PASS: pathway labels follow phase/energy rules (mismatches=0)
- PASS: pathway ids match pathway labels (mismatches=0)

## Evolution Snapshot
- ticks: 0 -> 88 (12 snapshots)
- nodes_per_tick: 512
- active_nodes: 127 -> 15
- first_tick_phase_counts: {"formation": 127, "free": 385}
- last_tick_phase_counts: {"dissolution": 3, "formation": 12, "free": 497}

## Totals Across Run
- rows: 6144
- phase_totals: {"dissolution": 35, "formation": 269, "free": 5840}
