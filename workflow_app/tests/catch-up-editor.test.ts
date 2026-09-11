import { afterEach, test } from 'node:test'
import assert from 'node:assert/strict'

import { setDatabaseTestExecutor } from '../../db/client'
import type { QueryExecutor } from '../../db/query-executor'
import { createTask } from '../../db/tasks'
import {
  CATCHUP_WORKSTREAMS,
  categoriesForWorkstream,
} from '../../lib/catchup/task-taxonomy'
import {
  PRIORITY_LEVELS,
  priorityLabel,
  priorityToLevel,
} from '../../lib/catchup/task-priority'

// CATCH-UP — Task Workspace editor proof (bounded).
//   - taxonomy: supported workstream/category sets (dependent dropdowns)
//   - priority: bounded 0=LOW / 1=MEDIUM / 2=HIGH convention
//   - createTask: canonical seam creates a context-free task with taxonomy
//   - workspace source: opens editable (no Edit gate), exposes taxonomy,
//     target/add dates, and NEW TASK / CREATE TASK

afterEach(() => setDatabaseTestExecutor(null))

type Captured = { sql: string; params: unknown[] }

function makeExecutor(
  sequences: Record<string, unknown>[][],
  captured: Captured[],
): QueryExecutor {
  let i = 0
  return async (strings, ...params) => {
    captured.push({ sql: strings.join('?'), params })
    const set = sequences[Math.min(i, sequences.length - 1)] ?? []
    i++
    return set
  }
}

test('taxonomy: supported workstream/category sets are constrained per workstream', () => {
  assert.deepEqual(CATCHUP_WORKSTREAMS, ['CLIENT', 'CORE', 'OPPS', 'SUPPORT', 'TECH'])
  assert.deepEqual(categoriesForWorkstream('CLIENT'), ['FOLLOWUP', 'ONBOARDING', 'CONTRACTS', 'MEDIA'])
  assert.deepEqual(categoriesForWorkstream('CORE'), ['ACCOUNTING', 'MARKETING', 'LEGAL', 'MANAGEMENT'])
  assert.deepEqual(categoriesForWorkstream('OPPS'), ['DATA_ENTRY'])
  assert.deepEqual(categoriesForWorkstream('SUPPORT'), ['SYSTEMS', 'SECURITY'])
  assert.deepEqual(categoriesForWorkstream('TECH'), ['NEW_TECH', 'INFRASTRUCTURE'])
  assert.deepEqual(categoriesForWorkstream('UNKNOWN'), [])
  // No cross-workstream category bleed.
  assert.ok(!categoriesForWorkstream('CLIENT').includes('SYSTEMS'))
})

test('priority: bounded 0=LOW / 1=MEDIUM / 2=HIGH mapping (no raw integer in UI)', () => {
  assert.equal(priorityToLevel(0), 0)
  assert.equal(priorityToLevel(1), 1)
  assert.equal(priorityToLevel(2), 2)
  assert.equal(priorityToLevel(-3), 0)
  assert.equal(priorityToLevel(7), 2)
  assert.equal(priorityLabel(0), 'LOW')
  assert.equal(priorityLabel(1), 'MEDIUM')
  assert.equal(priorityLabel(2), 'HIGH')
  assert.deepEqual(PRIORITY_LEVELS.map((p) => p.label), ['LOW', 'MEDIUM', 'HIGH'])
})

test('createTask: creates a context-free task with workstream/category through the canonical seam', async () => {
  const captured: Captured[] = []
  setDatabaseTestExecutor(
    makeExecutor(
      [
        [
          {
            id: 'new-1',
            title: 'Call client',
            detail: null,
            person_id: null,
            property_id: null,
            deal_id: null,
            source_interaction_id: null,
            assigned_user_id: null,
            due_at: '2026-09-01T00:00:00.000Z',
            task_kind: 'human',
            priority: 1,
            status: 'open',
            completed_at: null,
            created_at: '2026-08-27T00:00:00.000Z',
            updated_at: '2026-08-27T00:00:00.000Z',
          },
        ],
      ],
      captured,
    ),
  )
  const task = await createTask({
    title: 'Call client',
    detail: 'notes',
    dueAt: '2026-09-01T00:00:00.000Z',
    priority: 1,
    workstream: 'CLIENT',
    category: 'CONTRACTS',
  })
  assert.equal(task.id, 'new-1')
  assert.equal(task.status, 'open')
  assert.equal(captured.length, 1)
  assert.ok(/insert into task/i.test(captured[0].sql))
  assert.ok(/workstream/.test(captured[0].sql))
  assert.ok(/category/.test(captured[0].sql))
  assert.ok(captured[0].params.includes('CLIENT'))
  assert.ok(captured[0].params.includes('CONTRACTS'))
})

// (The workspace source guard that read components/portal/catch-up-task-detail.tsx
// was removed with the Catch-Up surface itself. The data-layer proofs above are
// the durable ones.)

