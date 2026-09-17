import assert from 'node:assert/strict'
import test from 'node:test'

import {
  adjudicateRulingRevision,
  checkQaRunVerdictConsistency,
  checkQaRulingRevision,
  createRulingReader,
} from '../forge/forge-qa-consistency'

// A tiny injected history. `b` is the candidate, `a` is its ancestor, `z` diverged from it. No git
// and no live HEAD: the proof drives the loaded-before case with an injected ancestor probe.
const history: Record<string, string[]> = {
  a: [],
  b: ['a'],
  z: [],
}

const isAncestor = (ancestor: string, descendant: string): boolean => {
  const seen = new Set<string>()
  const stack = [descendant]
  while (stack.length > 0) {
    const rev = stack.pop() as string
    if (seen.has(rev)) continue
    seen.add(rev)
    for (const parent of history[rev] ?? []) {
      if (parent === ancestor) return true
      stack.push(parent)
    }
  }
  return false
}

test('a ruling names the revision the reader ran', () => {
  const reader = createRulingReader({ revision: 'a', isAncestor })
  assert.equal(reader.revision, 'a')

  const ruling = reader.adjudicate('a')
  assert.equal(ruling.state, 'fresh')
  if (ruling.state !== 'fresh') return
  assert.equal(ruling.readerRevision, 'a')
  assert.equal(ruling.candidateRevision, 'a')
})

test('a ruling from code older than the candidate is stale', () => {
  const ruling = adjudicateRulingRevision({ readerRevision: 'a', candidateRevision: 'b', isAncestor })
  assert.equal(ruling.state, 'stale')
  if (ruling.state !== 'stale') return
  assert.equal(ruling.readerRevision, 'a')
  assert.equal(ruling.candidateRevision, 'b')
  assert.match(ruling.detail, /stale/)
})

test('a stale ruling is not reported as a verdict on the candidate', () => {
  const result = checkQaRulingRevision({
    runStatus: 'Complete',
    verdict: true,
    readerRevision: 'a',
    candidateRevision: 'b',
    isAncestor,
  })
  assert.equal(result.state, 'stale')
  assert.ok(!('consistency' in result))
  assert.ok(!('verdict' in result))
})

test('a fresh reader judging the same candidate and proof is not flagged', () => {
  const reader = createRulingReader({ revision: 'b', isAncestor })
  assert.equal(reader.adjudicate('b').state, 'fresh')

  const result = checkQaRulingRevision({
    runStatus: 'Complete',
    verdict: true,
    readerRevision: 'b',
    candidateRevision: 'b',
    isAncestor,
  })
  assert.equal(result.state, 'fresh')
  if (result.state !== 'fresh') return
  assert.deepEqual(
    result.consistency,
    checkQaRunVerdictConsistency({ runStatus: 'Complete', verdict: true }),
  )
})

test('a reader loaded before the change it judges is flagged stale', () => {
  // The reader is LOADED at `a` before candidate `b` exists. It keeps that revision even when it is
  // asked to judge the newer candidate, so the ruling is stale rather than a verdict on `b`.
  const reader = createRulingReader({ revision: 'a', isAncestor })
  const ruling = reader.adjudicate('b')
  assert.equal(reader.revision, 'a')
  assert.equal(ruling.state, 'stale')
})

test('a divergent or unreadable revision is unknown, never fresh', () => {
  const divergent = adjudicateRulingRevision({ readerRevision: 'z', candidateRevision: 'b', isAncestor })
  assert.equal(divergent.state, 'unknown')

  for (const readerRevision of [null, '', undefined]) {
    const blank = adjudicateRulingRevision({ readerRevision, candidateRevision: 'b', isAncestor })
    assert.equal(blank.state, 'unknown')
  }
})
