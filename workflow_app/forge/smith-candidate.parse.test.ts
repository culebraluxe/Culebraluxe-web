import assert from 'node:assert/strict'
import test from 'node:test'
import {
  parseSmithCandidate,
  SMITH_CANDIDATE_MISSING,
  SMITH_CANDIDATE_PREFIX,
} from './smith-candidate-parse'

test('parses one SMITH_CANDIDATE line', () => {
  const notes = `${SMITH_CANDIDATE_PREFIX} {"version":1,"assignmentId":"a1","candidateSha":"aaaaaaaa","mergeBase":"bbbbbbbb","changedPaths":["workflow_app/forge/a.ts"]}`
  const c = parseSmithCandidate(notes)
  assert.equal(c?.assignmentId, 'a1')
  assert.equal(c?.candidateSha, 'aaaaaaaa')
  assert.deepEqual(c?.changedPaths, ['workflow_app/forge/a.ts'])
})

test('duplicate or broken lines fail closed', () => {
  const line = `${SMITH_CANDIDATE_PREFIX} {"version":1,"assignmentId":"a1","candidateSha":"aaaaaaaa","mergeBase":"bbbbbbbb","changedPaths":["a.ts"]}`
  assert.equal(parseSmithCandidate(line + '\n' + line), null)
  assert.equal(parseSmithCandidate('SMITH_CANDIDATE: {broken'), null)
  assert.equal(parseSmithCandidate('no marker'), null)
  assert.ok(SMITH_CANDIDATE_MISSING.includes('SMITH_CANDIDATE'))
})
