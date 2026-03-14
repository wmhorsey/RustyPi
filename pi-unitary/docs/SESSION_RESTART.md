# Session Restart (60 Seconds)

Use this when context compression drops scope.

## Step 1: Read in Order

1. [PROJECT_KERNEL.md](PROJECT_KERNEL.md)
2. [HYPOTHESIS_LEDGER.md](HYPOTHESIS_LEDGER.md)
3. [EVIDENCE_LOG.md](EVIDENCE_LOG.md)

## Step 2: Restore Last Ground Truth

1. Open [reports/clean_room/ontology_check.md](../reports/clean_room/ontology_check.md)
2. Confirm last PASS/FAIL state.

## Step 3: Pick Exactly One Next Question

Use this sentence form:
- Does change X increase/decrease metric Y while preserving conservation and no core leak?

## Step 4: Run One Experiment Only

1. Make one mechanism change.
2. Run one canonical test.
3. Append result to [EVIDENCE_LOG.md](EVIDENCE_LOG.md).

## Step 5: Stop

Do not branch into multiple rewrites in one session.
