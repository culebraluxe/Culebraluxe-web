import assert from 'node:assert/strict'
import test from 'node:test'

import { normalizeAgentFinishForRole } from './repositories'

const complete = {
  resultStatus: 'Complete',
  completion: 100,
  notes: 'assay finished',
  commitHash: 'should-not-survive-assay',
  testsSummary: 'all checks passed',
}

test('Smith/builder finish behavior is unchanged', () => {
  assert.deepEqual(normalizeAgentFinishForRole('builder', complete), complete)
})

test('Assay clean pass preserves Complete but cannot keep a commit', () => {
  assert.deepEqual(normalizeAgentFinishForRole('reviewer', complete), {
    ...complete,
    commitHash: null,
  })
})

test('Assay non-Complete result becomes Hold before story finish', () => {
  assert.deepEqual(
    normalizeAgentFinishForRole('reviewer', {
      ...complete,
      resultStatus: 'Partial',
      completion: 70,
    }),
    {
      ...complete,
      resultStatus: 'Hold',
      completion: 70,
      commitHash: null,
    },
  )
})

test('Assay Complete with failed tests becomes Hold before story finish', () => {
  const normalized = normalizeAgentFinishForRole('reviewer', {
    ...complete,
    testsSummary: '1 failed, 12 passed',
  })
  assert.equal(normalized.resultStatus, 'Hold')
  assert.equal(normalized.commitHash, null)
})

test('Assay Complete with violation or policy evidence becomes Hold', () => {
  for (const testsSummary of ['verification violation', 'policy gate rejected']) {
    assert.equal(
      normalizeAgentFinishForRole('reviewer', {
        ...complete,
        testsSummary,
      }).resultStatus,
      'Hold',
    )
  }
})
