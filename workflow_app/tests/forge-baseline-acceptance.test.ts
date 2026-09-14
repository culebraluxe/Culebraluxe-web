import assert from 'node:assert/strict'
import test from 'node:test'
import { readFileSync } from 'node:fs'

import { assessBaselineAcceptance } from '../forge/baseline-acceptance'

// ---------------------------------------------------------------------------
// A STORY MUST BE FALSIFIABLE.
//
// Found by the difficulty ladder: a story whose frozen proof already exits 0 at the base
// commit has acceptance that holds before any work, so an unrelated change passes QA. The
// proof measured "does the acceptance hold", not "did this story do anything".
// ---------------------------------------------------------------------------

test('baseline: all proofs green at base means the story cannot be worked', () => {
  const verdict = assessBaselineAcceptance([
    { command: 'node --test a.test.ts', exitCode: 0 },
    { command: 'node --test b.test.ts', exitCode: 0 },
  ])
  assert.equal(verdict.satisfiedAtBase, true)
  if (!verdict.satisfiedAtBase) return
  assert.equal(verdict.passed, 2)
  assert.equal(verdict.total, 2)
  assert.match(verdict.reason, /BASELINE ACCEPTANCE/)
  assert.match(verdict.reason, /2\/2/)
  assert.match(verdict.reason, /falsifiable/i)
  assert.match(verdict.reason, /node --test a\.test\.ts/, 'the evidence names the proofs')
})

test('baseline: ONE failing proof means there is real work, so the lane may start', () => {
  const verdict = assessBaselineAcceptance([
    { command: 'node --test a.test.ts', exitCode: 0 },
    { command: 'node --test b.test.ts', exitCode: 1 },
  ])
  assert.equal(verdict.satisfiedAtBase, false)
  assert.equal(verdict.passed, 1)
  assert.equal(verdict.total, 2)
})

test('baseline: an unexecutable proof is NOT a pass', () => {
  const verdict = assessBaselineAcceptance([
    { command: 'node --test a.test.ts', exitCode: 0 },
    { command: 'missing-binary --test', exitCode: null, unmeasurable: true },
  ])
  assert.equal(verdict.satisfiedAtBase, false, 'unmeasurable must never satisfy acceptance')
})

test('baseline: no proofs at all is unverifiable, not satisfied', () => {
  const verdict = assessBaselineAcceptance([])
  assert.equal(verdict.satisfiedAtBase, false)
  assert.equal(verdict.total, 0)
})

// --- the wiring fence -------------------------------------------------------

const RUNNER = readFileSync(
  new URL('../forge/agent-runtime-role-runner.ts', import.meta.url),
  'utf8',
)

test('baseline: the runner refuses to start a code lane on already-satisfied acceptance', () => {
  const doorAt = RUNNER.indexOf('DOOR 3 — BASELINE ACCEPTANCE')
  const assessAt = RUNNER.indexOf('assessBaselineAcceptance(results)')
  const runAt = RUNNER.indexOf('runAssayCommand({')

  assert.ok(doorAt > 0, 'DOOR 3 must exist')
  assert.ok(assessAt > doorAt, 'the verdict is computed inside DOOR 3')
  assert.ok(runAt > doorAt && runAt < assessAt, 'the proofs are run, then assessed')
  assert.match(RUNNER, /if \(baseline\.satisfiedAtBase\) throw new Error\(baseline\.reason\)/)
  assert.match(
    RUNNER,
    /writesCode && attempt === 0 && workspaces\?\.worktreesRoot/,
    'only the first attempt of a code-writing lane',
  )
  assert.match(
    RUNNER,
    /rev-list', '--count', `\$\{workspaces\.baseRef\}\.\.HEAD`/,
    'the baseline is decided by git, and the ref is allowed to have moved on',
  )
  assert.match(
    RUNNER,
    /leadRoutingContext\.allowedProofs\.filter\(\(command\) => command\.trim\(\)\)/,
    'the proofs are the story frozen commands, never model prose',
  )
})
