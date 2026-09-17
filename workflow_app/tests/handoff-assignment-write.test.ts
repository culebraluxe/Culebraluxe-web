import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'
import { test } from 'node:test'

import { decideAssignmentWrite } from '../../db/forge-role-assignment-write'

// ---------------------------------------------------------------------------
// THE PLAN WRITE'S ONE DECISION (2026-09-17).
//
// `scripts/forge-handoff.mjs` upserted the assignment's finding_ids with a
// `cardinality > 0` replace, so each chunk write overwrote the last. Three chunks
// with disjoint finding sets on ONE assignment left all but the last unassigned.
// These tests pin the decider's rule and the write path's use of it, with no
// database: the decider is pure, so a fold over three writes IS the failure.
// ---------------------------------------------------------------------------

const write = (assignmentId: string, attempt: number, findingIds: string[]) => ({
  assignmentId,
  attempt,
  findingIds,
})

test('decider: a first write against no existing row is allowed whole', () => {
  assert.deepEqual(decideAssignmentWrite(null, write('a', 1, ['F1', 'F2'])), {
    kind: 'allow',
    findingIds: ['F1', 'F2'],
  })
})

test('decider: an additive write is allowed and the stored set is the union', () => {
  const decision = decideAssignmentWrite(write('a', 1, ['F1', 'F2']), write('a', 1, ['F3']))
  assert.deepEqual(decision, { kind: 'allow', findingIds: ['F1', 'F2', 'F3'] })
})

test('decider: replaying the identical write changes nothing and is allowed', () => {
  const decision = decideAssignmentWrite(write('a', 1, ['F1', 'F2']), write('a', 1, ['F2', 'F1']))
  assert.deepEqual(decision, { kind: 'allow', findingIds: ['F1', 'F2'] })
})

test('decider: a write that declares no finding is a no-op, not an erase', () => {
  const decision = decideAssignmentWrite(write('a', 1, ['F1', 'F2']), write('a', 1, []))
  assert.deepEqual(decision, { kind: 'allow', findingIds: ['F1', 'F2'] })
})

test('decider: a shrink is refused and names the assignment, attempt and every dropped finding', () => {
  const decision = decideAssignmentWrite(
    write('a', 1, ['F1', 'F2', 'F3']),
    write('a', 1, ['F1']),
  )
  assert.deepEqual(decision, {
    kind: 'refuse',
    assignmentId: 'a',
    attempt: 1,
    dropped: ['F2', 'F3'],
  })
})

test('the 2026-09-17 failure: three disjoint chunk writes on ONE assignment bind all of them', () => {
  const chunks = [
    ['F1', 'F2'],
    ['F3', 'F4'],
    ['F5', 'F6'],
  ]
  let existing: { assignmentId: string; attempt: number; findingIds: string[] } | null = null
  for (const chunk of chunks) {
    const decision = decideAssignmentWrite(existing, write('a', 1, chunk))
    assert.equal(decision.kind, 'allow')
    if (decision.kind === 'allow') {
      existing = { assignmentId: 'a', attempt: 1, findingIds: decision.findingIds }
    }
  }
  assert.deepEqual(existing?.findingIds, ['F1', 'F2', 'F3', 'F4', 'F5', 'F6'])
  for (const id of ['F1', 'F2', 'F3', 'F4', 'F5', 'F6']) {
    assert.ok(existing?.findingIds.includes(id), `${id} must not read as unassigned`)
  }
})

test('the write path calls the decider before the assignment upsert and no longer replaces', () => {
  const source = readFileSync(new URL('../../scripts/forge-handoff.mjs', import.meta.url), 'utf8')
  const callAt = source.indexOf('decideAssignmentWrite(')
  const upsertAt = source.indexOf('insert into forge_role_assignment')
  assert.ok(callAt >= 0, 'the write path must call decideAssignmentWrite')
  assert.ok(upsertAt >= 0, 'the assignment upsert must still exist')
  assert.ok(callAt < upsertAt, 'the decider must run before the assignment row is written')
  assert.match(source, /\.kind === 'refuse'/)
  assert.match(source, /finding_ids = excluded\.finding_ids/)
})
