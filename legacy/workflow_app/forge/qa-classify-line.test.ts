import assert from 'node:assert/strict'
import test from 'node:test'
import { attachFailureClass, parseFailureClass } from '@/legacy/workflow_app/forge/qa-classify-line'

test('reads FAILURE_CLASS after Assay FAIL', () => {
  assert.equal(parseFailureClass('FAILURE_CLASS: TEST_DEFECT'), 'TEST_DEFECT')
  assert.equal(parseFailureClass('FAILURE_CLASS: nope'), null)
})

test('does not attach a class on PASS', () => {
  const tagged = attachFailureClass({ verdict: 'PASS', failureClass: undefined }, 'FAILURE_CLASS: CODE_DEFECT')
  assert.equal(tagged.failureClass, null)
})
