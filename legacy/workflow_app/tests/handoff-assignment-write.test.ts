import assert from 'node:assert/strict'
import { test } from 'node:test'

import { decideAssignmentWrite, decideContractWrite } from '@/legacy/db/forge-role-assignment-write'

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

// CONTRACT ROW (2026-09-17). `forge_role_contract` carried the same last-non-empty-wins rule for
// finding_ids, merge_checks and surface_scope. These tests pin the contract decider: one rule,
// reused, with each column decided against its own value. The source-grep assertion is gone —
// a fold over the writes is the only verdict.

test('contract: a first write against no existing row is allowed whole', () => {
  assert.deepEqual(
    decideContractWrite(null, { findingIds: ['F1'], mergeChecks: ['node --test a'], surfaceScope: [] }),
    { kind: 'allow', findingIds: ['F1'], mergeChecks: ['node --test a'], surfaceScope: [] },
  )
})

test('contract: an additive write unions each column', () => {
  const decision = decideContractWrite(
    { findingIds: ['F1'], mergeChecks: ['m1'], surfaceScope: ['legacy/workflow_app/a.ts'] },
    { findingIds: ['F2'], mergeChecks: ['m2'], surfaceScope: ['legacy/workflow_app/b.ts'] },
  )
  assert.deepEqual(decision, {
    kind: 'allow',
    findingIds: ['F1', 'F2'],
    mergeChecks: ['m1', 'm2'],
    surfaceScope: ['legacy/workflow_app/a.ts', 'legacy/workflow_app/b.ts'],
  })
})

test('contract: a write that declares nothing is a no-op, not an erase', () => {
  const decision = decideContractWrite(
    { findingIds: ['F1'], mergeChecks: ['m1'], surfaceScope: ['legacy/workflow_app/a.ts'] },
    { findingIds: [], mergeChecks: [], surfaceScope: [] },
  )
  assert.deepEqual(decision, {
    kind: 'allow',
    findingIds: ['F1'],
    mergeChecks: ['m1'],
    surfaceScope: ['legacy/workflow_app/a.ts'],
  })
})

test('contract: a shrinking column is refused by name with its dropped entries', () => {
  const decision = decideContractWrite(
    { findingIds: ['F1', 'F2'], mergeChecks: ['m1', 'm2'], surfaceScope: ['s1', 's2'] },
    { findingIds: ['F1'], mergeChecks: ['m1', 'm2'], surfaceScope: ['s1', 's2'] },
  )
  assert.deepEqual(decision, { kind: 'refuse', column: 'finding_ids', dropped: ['F2'] })
})

test('contract: an add in one column cannot mask a shrink in another', () => {
  const decision = decideContractWrite(
    { findingIds: ['F1'], mergeChecks: ['m1', 'm2'], surfaceScope: ['s1'] },
    { findingIds: ['F2'], mergeChecks: ['m1'], surfaceScope: ['s1'] },
  )
  assert.deepEqual(decision, { kind: 'refuse', column: 'merge_checks', dropped: ['m2'] })
})

test('contract: a shrink of surface_scope is refused, so a surface cannot silently disappear', () => {
  const decision = decideContractWrite(
    { findingIds: [], mergeChecks: [], surfaceScope: ['legacy/workflow_app/a.ts', 'legacy/workflow_app/b.ts'] },
    { findingIds: [], mergeChecks: [], surfaceScope: ['legacy/workflow_app/a.ts'] },
  )
  assert.deepEqual(decision, {
    kind: 'refuse',
    column: 'surface_scope',
    dropped: ['legacy/workflow_app/b.ts'],
  })
})

test('contract: the 2026-09-17 fold — three disjoint writes bind every column of one row', () => {
  const writes = [
    { findingIds: ['F1'], mergeChecks: ['m1'], surfaceScope: ['s1'] },
    { findingIds: ['F2'], mergeChecks: ['m2'], surfaceScope: ['s2'] },
    { findingIds: ['F3'], mergeChecks: ['m3'], surfaceScope: ['s3'] },
  ]
  let existing: { findingIds: string[]; mergeChecks: string[]; surfaceScope: string[] } | null = null
  for (const write of writes) {
    const decision = decideContractWrite(existing, write)
    assert.equal(decision.kind, 'allow')
    if (decision.kind === 'allow') {
      existing = {
        findingIds: decision.findingIds,
        mergeChecks: decision.mergeChecks,
        surfaceScope: decision.surfaceScope,
      }
    }
  }
  assert.deepEqual(existing, {
    findingIds: ['F1', 'F2', 'F3'],
    mergeChecks: ['m1', 'm2', 'm3'],
    surfaceScope: ['s1', 's2', 's3'],
  })
})
