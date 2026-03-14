# Evidence Log (Append-Only)

Purpose: preserve session continuity under context compression.

Format per entry:
1. Date
2. Question
3. Change (single mechanism)
4. Run IDs
5. Metrics
6. Decision (PASS/FAIL)
7. Next single action

---

## 2026-03-13

Question:
Can geometry-forced blocked transport at a void boundary produce shell intensification while conserving STE and keeping core leakage at zero?

Change:
Added clean-room deterministic single-field ontology checker.

Run IDs:
1. clean-room control + void from [reports/clean_room/ontology_check.json](../reports/clean_room/ontology_check.json)

Metrics:
1. conservation_error_max = 0
2. core_leak_samples = 0
3. redirection_ratio = 1.0
4. shell_to_far_peak_ratio = 22.2956 (void)
5. shell_emergence_latency = 0

Decision:
PASS

Next single action:
Run quantization sensitivity sweep on move_div values 16, 24, 32, 48 and test metric ordering stability.
