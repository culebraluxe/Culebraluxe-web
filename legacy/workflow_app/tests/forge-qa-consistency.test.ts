import assert from 'node:assert/strict'
import test from 'node:test'

import {
  checkQaRunVerdictConsistency,
  expectedVerdictForRunStatus,
  isQaRunType,
  normalizeQaVerdict,
  renderQaConsistencyLine,
} from '@/legacy/workflow_app/forge/forge-qa-consistency'

test('a clean run and a PASS verdict agree', () => {
  assert.deepEqual(checkQaRunVerdictConsistency({ runStatus: 'Complete', verdict: true }), {
    state: 'agree',
    runStatus: 'Complete',
    verdict: 'PASS',
  })
})

test('a failed run and a FAIL verdict agree', () => {
  assert.deepEqual(checkQaRunVerdictConsistency({ runStatus: 'Failed', verdict: false }), {
    state: 'agree',
    runStatus: 'Failed',
    verdict: 'FAIL',
  })
})

test('a run status and a verdict that disagree name BOTH values', () => {
  const complete = checkQaRunVerdictConsistency({ runStatus: 'Complete', verdict: false })
  assert.equal(complete.state, 'disagree')
  if (complete.state !== 'disagree') return
  assert.equal(complete.runStatus, 'Complete')
  assert.equal(complete.verdict, 'FAIL')
  assert.match(complete.detail, /Complete/)
  assert.match(complete.detail, /FAIL/)

  const failed = checkQaRunVerdictConsistency({ runStatus: 'Failed', verdict: true })
  assert.equal(failed.state, 'disagree')
  if (failed.state !== 'disagree') return
  assert.match(failed.detail, /Failed/)
  assert.match(failed.detail, /PASS/)
})

test('a run that made no clean pass/fail claim and still has a verdict disagrees', () => {
  for (const verdict of [true, false]) {
    const result = checkQaRunVerdictConsistency({ runStatus: 'Hold', verdict })
    assert.equal(result.state, 'disagree')
    if (result.state !== 'disagree') continue
    assert.equal(result.runStatus, 'Hold')
    assert.match(result.detail, /Hold/)
  }
})

test('a run with no verdict reports unknown — never agree', () => {
  for (const verdict of [null, undefined, '']) {
    const result = checkQaRunVerdictConsistency({ runStatus: 'Complete', verdict })
    assert.equal(result.state, 'unknown')
  }
  const noStatus = checkQaRunVerdictConsistency({ verdict: null })
  assert.equal(noStatus.state, 'unknown')
})

test('a verdict token is read case-insensitively and an unreadable one is no verdict', () => {
  assert.equal(normalizeQaVerdict('pass'), 'PASS')
  assert.equal(normalizeQaVerdict(' FAIL '), 'FAIL')
  assert.equal(normalizeQaVerdict(true), 'PASS')
  assert.equal(normalizeQaVerdict(false), 'FAIL')
  assert.equal(normalizeQaVerdict('maybe'), null)
  assert.equal(normalizeQaVerdict(null), null)
})

test('only Complete and Failed imply a verdict', () => {
  assert.equal(expectedVerdictForRunStatus('Complete'), 'PASS')
  assert.equal(expectedVerdictForRunStatus('Failed'), 'FAIL')
  assert.equal(expectedVerdictForRunStatus('Hold'), null)
  assert.equal(expectedVerdictForRunStatus('Cancelled'), null)
  assert.equal(expectedVerdictForRunStatus(null), null)
})

test('QA lane runs are the qa/assay family', () => {
  assert.equal(isQaRunType('qa'), true)
  assert.equal(isQaRunType('qa_verify'), true)
  assert.equal(isQaRunType('fast_qa_verify'), true)
  assert.equal(isQaRunType('assay'), true)
  assert.equal(isQaRunType('smith'), false)
  assert.equal(isQaRunType('scout'), false)
  assert.equal(isQaRunType(null), false)
})

test('the rendered disagreement line names both values', () => {
  const result = checkQaRunVerdictConsistency({ runStatus: 'Hold', verdict: true })
  const line = renderQaConsistencyLine(result)
  assert.match(line, /DISAGREE/)
  assert.match(line, /Hold/)
  assert.match(line, /PASS/)

  const unknown = renderQaConsistencyLine(
    checkQaRunVerdictConsistency({ runStatus: 'Complete', verdict: null }),
  )
  assert.match(unknown, /unknown/)
  assert.doesNotMatch(unknown, /agree/)
})
