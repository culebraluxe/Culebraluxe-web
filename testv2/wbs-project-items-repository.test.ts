import { test } from 'node:test'
import assert from 'node:assert/strict'

import { SqlWbsRepository } from '../db/wbs-service-repository'
import type { QueryExecutor } from '../db/query-executor'

test('SQL project-items query returns terminal rows and orders by persisted sibling order', async () => {
  let statement = ''
  const execute: QueryExecutor = async (strings) => {
    statement = strings.join('?').replace(/\s+/g, ' ').trim().toLowerCase()
    return [
      {
        id: 'done-parent',
        project_id: 'project-1',
        parent_id: null,
        title: 'Listing Agreement',
        notes: '',
        category: 'contracts',
        status: 'done',
        due_at: null,
        owner: 'user-1',
        sort_order: 1,
        entity_type: 'contract',
        entity_id: 'contract-1',
        created_at: '2026-09-09T00:00:00.000Z',
        updated_at: '2026-09-09T00:00:00.000Z',
      },
      {
        id: 'open-child',
        project_id: 'project-1',
        parent_id: 'done-parent',
        title: 'Seller Signature',
        notes: '',
        category: 'contracts',
        status: 'open',
        due_at: null,
        owner: 'user-1',
        sort_order: 1,
        entity_type: null,
        entity_id: null,
        created_at: '2026-09-09T00:00:00.000Z',
        updated_at: '2026-09-09T00:00:00.000Z',
      },
    ]
  }

  const items = await new SqlWbsRepository(execute).listProjectItems({})

  assert.deepEqual(items.map((item) => item.status), ['done', 'open'])
  assert.equal(items[1]?.parentId, 'done-parent')
  assert.match(statement, /where project_id is not null/)
  assert.doesNotMatch(statement, /status in/)
  assert.match(statement, /sort_order nulls last/)
})
