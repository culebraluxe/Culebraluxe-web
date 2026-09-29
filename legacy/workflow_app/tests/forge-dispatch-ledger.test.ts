import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import {
  DIFFICULTY_BIAS,
  DIFFICULTY_WEIGHTS,
  LogisticScorer,
  SCORER_ID,
  difficultyLogit,
  sigmoid,
} from '@/legacy/workflow_app/forge/forge-difficulty-scorer'
import {
  TELEMETRY_DEFAULT_FEATURES,
  difficultyFeaturesForPlanDetailed,
  scorePlanDetailed,
} from '@/legacy/workflow_app/forge/forge-plan-difficulty'

// ---------------------------------------------------------------------------
// THE DISPATCH LEDGER (migration 175) — the dataset the difficulty model needs.
//
// `forge-difficulty-scorer.ts` predicts p_success with hand-set weights and says they are
// "provisional calibration food" for a fit learned from run history. Nothing was ever
// recorded, so there was no history to learn from. These tests fence the two things that
// make a fit possible at all:
//   1. the RECORD must be the model's own numbers (same logit, same features, same
//      measurement provenance), never a lookalike recomputed somewhere else;
//   2. the LABEL (outcome) must attach to the SAME unit the prediction was made about.
// ---------------------------------------------------------------------------

const PLAN = {
  chunks: [
    {
      id: 1,
      surface: ['legacy/workflow_app/tests/a.test.ts'],
      proof: 'node --import tsx --test workflow_app/tests/a.test.ts',
      dependsOn: null,
    },
  ],
}

test('ledger: the recorded logit is the logit the prediction was made from', () => {
  const scored = scorePlanDetailed(PLAN)
  assert.equal(
    Number(scored.pSuccess.toFixed(10)),
    Number(sigmoid(difficultyLogit(scored.features)).toFixed(10)),
    'p_success must be reproducible from the recorded logit',
  )
  assert.equal(Number(scored.logit.toFixed(6)), Number(difficultyLogit(scored.features).toFixed(6)))
})

test('ledger: a scorer with different weights still records ITS OWN logit', () => {
  // A future fitted scorer must not be described by the old weights: the ledger records the
  // number that produced the score, whatever scorer produced it.
  const doubled = Object.fromEntries(
    Object.entries(DIFFICULTY_WEIGHTS).map(([k, v]) => [k, v * 2]),
  ) as Record<keyof typeof DIFFICULTY_WEIGHTS, number>
  const scorer = new LogisticScorer(doubled, DIFFICULTY_BIAS)
  const scored = scorePlanDetailed(PLAN, scorer)
  assert.equal(
    Number(scored.pSuccess.toFixed(10)),
    Number(sigmoid(difficultyLogit(scored.features, doubled, DIFFICULTY_BIAS)).toFixed(10)),
  )
  assert.notEqual(
    Number(scored.logit.toFixed(4)),
    Number(difficultyLogit(scored.features).toFixed(4)),
  )
  assert.equal(SCORER_ID, 'logistic-hand-weighted-v1')
})

test('ledger: the record says which features were MEASURED and which were defaults', () => {
  const { measured } = difficultyFeaturesForPlanDetailed(PLAN)
  assert.deepEqual(
    measured.slice().sort(),
    ['depDepth', 'filesTouched', 'genericType', 'hasAcceptance'],
    'the four plan-derived features are the measured ones',
  )
  for (const name of TELEMETRY_DEFAULT_FEATURES) {
    assert.equal(
      measured.includes(name),
      false,
      `${name} is a neutral default and must not be recorded as evidence`,
    )
  }
  assert.equal(
    measured.length + TELEMETRY_DEFAULT_FEATURES.length,
    Object.keys(DIFFICULTY_WEIGHTS).length,
  )
})

// --- the writers, fenced at the SQL boundary -------------------------------
//
// The recorder needs a live table, so the shape is asserted here: the prediction upserts on
// the unit identity, and the label updates THAT row while preserving anything it does not
// set. `null` means unmeasured — never a wipe.

const LEDGER = readFileSync(new URL('../../db/forge-dispatch-score.ts', import.meta.url), 'utf8')

test('ledger: the prediction upserts on the unit identity, so a re-score cannot stack rows', () => {
  assert.match(LEDGER, /insert into forge_dispatch_score/)
  assert.match(
    LEDGER,
    /on conflict \(task_id, node_id, attempt, assignment_id, chunk_id\) do update set/,
    'one row per assessed unit per attempt',
  )
  for (const field of [
    'files_touched',
    'loc_ratio',
    'dep_depth',
    'has_acceptance',
    'context_ratio',
    'historical_success',
    'repo_size_bucket',
    'generic_type',
    'measured_features',
    'scorer_id',
    'logit',
    'p_success',
    'gate',
  ]) {
    assert.ok(LEDGER.includes(field), `the record must carry ${field}`)
  }
})

test('ledger: the label does not filter by the EXECUTING node, or it misses its own rows', () => {
  // The prediction is recorded at lead_pre; the result arrives from smith/lead_solo_implement.
  // Filtering on the executing node made every label miss (observed live), so the identity is
  // the assignment and the node filter is optional and normally absent.
  const outcome = LEDGER.slice(LEDGER.indexOf('export async function recordForgeDispatchOutcome'))
  assert.match(
    outcome,
    /and \(\$\{input\.nodeId \?\? null\}::text is null or node_id = \$\{input\.nodeId \?\? null\}\)/,
    'the node predicate must be optional',
  )
  assert.match(RUNNER, /the unit is the ASSIGNMENT/, 'the runner must say why it omits the node')
})

test('ledger: the label attaches to the same unit and preserves what it does not set', () => {
  const outcome = LEDGER.slice(LEDGER.indexOf('export async function recordForgeDispatchOutcome'))
  assert.match(outcome, /update forge_dispatch_score set/)
  assert.match(outcome, /where task_id = \$\{input\.taskId\}/)
  assert.match(outcome, /and chunk_id = \$\{input\.chunkId \?\? 0\}/)
  assert.match(
    outcome,
    /coalesce\(\$\{input\.turns \?\? null\}, turns\)/,
    'an absent measurement must not overwrite a recorded one',
  )
  assert.match(outcome, /returning id/)
})

test('ledger: the reader can ask for only LABELLED rows, so a fit cannot learn from predictions alone', () => {
  assert.match(LEDGER, /labelledOnly/)
  assert.match(LEDGER, /outcome is not null/)
})

// --- the wiring fences -----------------------------------------------------
//
// A ledger nobody writes to is just a schema. These read the runner because the behaviour
// needs a live control plane.

const RUNNER = readFileSync(
  new URL('../forge/agent-runtime-role-runner.ts', import.meta.url),
  'utf8',
)

test('ledger: the runner records the prediction at dispatch and the label at the Smith exit', () => {
  const predictAt = RUNNER.indexOf('recordForgeDispatchScore({')
  const labelAt = RUNNER.indexOf('recordForgeDispatchOutcome({')
  const exitAt = RUNNER.indexOf('assessSmithExit({')

  assert.ok(predictAt > 0, 'the runner must record predictions')
  assert.ok(labelAt > 0, 'the runner must record outcomes')
  assert.ok(predictAt < labelAt, 'a prediction is recorded before the label for it')
  assert.ok(exitAt > 0 && exitAt < labelAt, 'the label comes from the Smith exit adjudication')
  assert.match(
    RUNNER,
    /if \(nodeId === 'lead_pre' && recordedPlan\) \{/,
    'predictions attach to the Lead assignment the gate assessed',
  )
})

test('ledger: the ledger and the turn cap agree on what a generation is', () => {
  // Both read the engine's own ledger for the process instance, so the turns recorded as an
  // outcome are the same number the cap enforces.
  assert.match(RUNNER, /turns: await countForgeGenerationTurns\(String\(task\.processInstanceId\)\)/)
})

