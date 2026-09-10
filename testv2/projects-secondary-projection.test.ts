import { test } from 'node:test'
import assert from 'node:assert/strict'
import { mapProjectCalendarItems } from '../ui/projects/secondary-projection'
import type { WbsItem } from '../services/wbs'

const item = (id: string, dueAt: string | null): WbsItem => ({
  id, title: id, notes: '', category: 'clients', status: 'open', projectId: 'p', parentId: null,
  dueAt, owner: null, order: null, entity: null, createdAt: null, updatedAt: null,
})

test('project calendar projection includes only valid persisted WBS due dates in stable order', () => {
  const result = mapProjectCalendarItems([
    item('later', '2026-09-12T00:00:00.000Z'),
    item('no-date', null),
    item('bad-date', 'not-a-date'),
    item('earlier', '2026-09-10T00:00:00.000Z'),
  ])
  assert.deepEqual(result.map((entry) => entry.id), ['earlier', 'later'])
})
