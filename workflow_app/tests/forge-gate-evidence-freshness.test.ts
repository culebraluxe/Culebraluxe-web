// ---------------------------------------------------------------------------
// GATE-EVIDENCE FRESHNESS — the wiring fence for the loop that cost a night.
//
// The runner snapshots `forge_workflow_evidence` ONCE per attempt (for Lead routing) and that
// snapshot was also used to project the gate facts for every node. So `qa_verify` computed
// `candidate && verified === candidate` against a `current.candidateSha` that predated the Smith's
// commit in the same attempt: `exact` came out false, `qaPassed` was written false, the engine took
// the fail branch to `repair_smith`, the repair produced the same candidate, and the loop ran to the
// turn cap.
//
// Measured live 2026-09-14 on ENG-FORGE-TURN-VISIBILITY-01 (instance 092ddab8): qa_verify reported
// `{"qaPassed":true,"candidateSha":"0483c314..."}` at 05:35:19, the router took the fail branch one
// second later, and the persisted row read `qa_passed=true` with `candidate_sha == qa_verified_sha`
// throughout. The data was never wrong; the snapshot was stale.
//
// This is a SOURCE fence on purpose: the defect is the ORDER of two calls, which a pure unit test of
// the mapping cannot observe (the mapping is correct given fresh input).
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

const read = (p: string) => readFile(new URL(p, import.meta.url), 'utf8')

test('GATE-FRESHNESS: the gate projection re-reads evidence before it projects', async () => {
  const src = await read('../forge/agent-runtime-role-runner.ts')

  // The projection must receive a freshly-read snapshot merged over the attempt-start context...
  assert.match(
    src,
    /const evidenceNow = await readForgeWorkflowEvidence\(/,
    'the runner must re-read evidence before projecting gate facts',
  )
  assert.match(
    src,
    /current: \{ \.\.\.current, \.\.\.evidenceNow \}/,
    'the projection must be given the fresh snapshot, with the attempt-start context as fallback',
  )
})

test('GATE-FRESHNESS: the projection is never handed the bare attempt-start snapshot', async () => {
  const src = await read('../forge/agent-runtime-role-runner.ts')

  // The exact shape that caused the loop: forgeEvidenceFromAgentResult({ ..., current, ... }).
  // A bare `current` here is the attempt-start snapshot and reproduces the QA -> repair loop.
  const call = src.slice(src.indexOf('forgeEvidenceFromAgentResult({'))
  const body = call.slice(0, call.indexOf('})'))
  assert.ok(
    !/\n\s*current,\n/.test(body),
    'passing the bare attempt-start `current` to the gate projection reintroduces the QA loop',
  )
})
