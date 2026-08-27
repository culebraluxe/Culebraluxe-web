import { afterEach, test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

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

test('workspace: opens editable (no Edit gate) with taxonomy/priority/date controls + create', () => {
  const src = readFileSync(
    new URL('../../components/portal/catch-up-task-detail.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/variant="feature"/.test(src), 'navy Feature workspace')
  assert.ok(/Task Workspace/.test(src), 'heading is Task Workspace')
  assert.ok(
    /saveTaskAction/.test(src) && /completeTaskAction/.test(src) && /createTaskAction/.test(src),
    'save / complete / create actions wired',
  )
  assert.ok(!/startEdit/.test(src), 'no Edit gate mode-switch')
  assert.ok(/name="title"/.test(src) && /name="detail"/.test(src), 'title + notes editable')
  assert.ok(/name="targetDate"/.test(src), 'Target Date field')
  assert.ok(/Add Date/.test(src) && /createdAt/.test(src), 'Add Date uses task.createdAt')
  assert.ok(/name="workstream"/.test(src) && /name="category"/.test(src), 'taxonomy dropdowns')
  assert.ok(/name="priority"/.test(src), 'priority selector')
  assert.ok(/PRIORITY_LEVELS/.test(src) && /priorityToLevel/.test(src), 'priority shows LOW/MEDIUM/HIGH, not integer')
  assert.ok(/New Task/.test(src) && /Create Task/.test(src), 'new/create task actions')
})
