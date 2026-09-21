# ENG-FORGE-DISPATCH-LEDGER-01 — write the metadata the stats math needs

## Why

`legacy/workflow_app/forge/forge-difficulty-scorer.ts` predicts p_success = σ(bias + w·x) over eight
features, with weights it calls "provisional calibration food, not claims", and its header
says a fit learned from run history should replace them. It could never be fitted, because
nothing recorded what the model predicted:

1. `assessSmithDispatch` computed `difficulty: {pSuccess, gate}` and the number reached only
   a reason string. No row.
2. `difficultyFeaturesForPlan` derived the vector at dispatch and discarded it. No row.
3. Four of the eight features (`locRatio`, `contextRatio`, `historicalSuccess`,
   `repoSizeBucket`) are documented NEUTRAL DEFAULTS, and nothing distinguished them from
   measurements — so any future fit would have learned from placeholders as though they were
   data.
4. The outcome was recorded at role-run granularity and never joined to the unit that was
   assessed, so prediction and result could not be paired.

A formula cannot be fitted against numbers nobody wrote down. This story starts writing them.

## Units

- **A** — `forge_dispatch_score` (migration 175): one row per assessed unit, holding the
  eight features, `measured_features` (which of the eight were actually measured this time),
  the scorer id, the logit, p_success, the gate verdict, the Lead route, and — filled in
  later — the observed outcome: verdict, repairs, turns, wall clock, cost, tokens, files
  changed, LOC delta, candidate SHA.
- **B** — `LogisticScorer.logit()` joins the `DifficultyScorer` protocol, and
  `scorePlanDetailed()` reads it FROM THE SCORER, so the recorded logit always explains the
  recorded probability. A scorer that cannot state its own linear predictor must decide what
  it writes there rather than inherit the reference logistic's number.
- **C** — the role runner records the prediction at `lead_pre` (per assignment, the
  granularity the prediction actually has), and the label at the Smith exit adjudication
  (`pass` when the candidate satisfied the doors, `fail` with the reasons otherwise).
- **D** — `null` is unmeasured, never zero, on both halves; the recorder is idempotent per
  `(task, node, attempt, assignment, chunk)` so a re-score updates rather than stacks.

## Acceptance

- Every assessed unit leaves a row carrying the features, the measurement provenance, the
  logit and the probability that produced the gate verdict.
- An outcome attaches to the SAME row and never overwrites a measurement with a null.
- `listForgeDispatchScores({ labelledOnly: true })` is the calibration set, and an empty
  slice is the honest answer while the ledger is young.

## Evidence (2026-09-13)

- `legacy/db/migrations/175_forge_dispatch_score.sql` (applied and verified DEV + PROD) ·
  `legacy/db/forge-dispatch-score.ts` · `legacy/workflow_app/forge/forge-difficulty-scorer.ts` (logit in
  the protocol) · `forge-plan-difficulty.ts` (scorePlanDetailed + measured provenance) ·
  `agent-runtime-role-runner.ts` (both writes) · `legacy/workflow_app/tests/forge-dispatch-ledger.test.ts`
  (8 cases, incl. the drift test that caught the logit/scorer mismatch).
- Proven against the real table inside a rolled-back transaction: prediction written, label
  attached, round-trip read `gate=dispatch p=0.9002 logit=2.2 turns=5
  measured=filesTouched,depDepth,hasAcceptance,genericType`, and a re-record kept ONE row
  while preserving the outcome.
- Suite 550 (547 pass, 0 fail), tsc clean.

## Not this story

Fitting the weights (ENG-FORGE-CALIBRATION-01) and filling the four telemetry defaults from
actuals. Both need ledger volume; inventing a model tonight against 12 vectors would be the
same error as trusting the hand-set weights.
