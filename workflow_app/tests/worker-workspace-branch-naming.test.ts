import assert from 'node:assert/strict'
import test from 'node:test'

import { deriveBranchName, sanitizeBranchSegment } from '../../lib/worker-workspace/provisioner'

// REGRESSION (2026-09-10 "mangled branch" incident, ENG-FORGE-SPLIT-01):
// a SPLIT child's run id is `<uuid 36>-e<generation>-split-<slot>`. The branch name
// truncated the run id to 40 chars, which cut the `-split-N` suffix off, so BOTH
// siblings derived the SAME branch (`…-e0-`) and the second child was refused as an
// attempt to steal another workspace. Two children must never share a branch.

const UUID = 'f8aa02a6-6cf7-4506-9a58-b8b648df7fe8' // 36 chars

test('split siblings derive DIFFERENT branch names (the suffix must survive)', () => {
  const a = deriveBranchName('eng-forge-split-dogfood-01', `${UUID}-e0-split-0`)
  const b = deriveBranchName('eng-forge-split-dogfood-01', `${UUID}-e0-split-1`)
  assert.notEqual(a, b)
  assert.match(a, /-split-0$/)
  assert.match(b, /-split-1$/)
})

test('serial run ids are unaffected by the split fix', () => {
  // `<uuid>-e0` is 39 chars: it was never truncated and must not change shape.
  const serial = deriveBranchName('some-story', `${UUID}-e0`)
  assert.equal(serial, `agent/some-story/${UUID}-e0`)
})

test('a replan generation also survives', () => {
  const g0 = deriveBranchName('s', `${UUID}-e0-split-0`)
  const g1 = deriveBranchName('s', `${UUID}-e1-split-0`)
  assert.notEqual(g0, g1)
})

test('sanitization still bounds the segment', () => {
  assert.equal(sanitizeBranchSegment('A/B C', 10), 'a-b-c')
})
