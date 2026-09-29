import { test } from 'node:test'
import assert from 'node:assert/strict'

import { portalJoinableIds } from '@/legacy/workflow_app/join-ids'

// THE ROW THAT CAUSED THIS: an engine instance whose subject_type is 'deal' and whose subject_id is not a uuid. The engine
// accepted it because `subject_id` is TEXT, and `where d.id = any(${ids}::uuid[])` then failed with 22P02 for the whole
// statement — taking down the system-health diagnostics snapshot and `getWorkflowSummaries` with it.
const THE_ROW = 'definitely-not-a-deal'

const VALID = '60000000-0000-4000-8000-000000000003'
const OTHER_VALID = 'eb7cbd90-2a9f-46a3-bb86-969f9dd09e65'

test('a non-uuid subject id is dropped instead of breaking the cast', () => {
  assert.deepEqual(portalJoinableIds([VALID, THE_ROW, OTHER_VALID]), [VALID, OTHER_VALID])
})

test('nothing that leaves here could fail ANY(...::uuid[])', () => {
  const out = portalJoinableIds([THE_ROW, '', '   ', 'null', 'undefined', '123', VALID])
  for (const id of out) {
    assert.match(id, /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i)
  }
})

test('absent subjects are dropped, because most instances have none', () => {
  assert.deepEqual(portalJoinableIds([null, undefined, null]), [])
  assert.deepEqual(portalJoinableIds([]), [])
})

test('duplicates are collapsed: the list is only a membership test', () => {
  assert.deepEqual(portalJoinableIds([VALID, VALID, OTHER_VALID, VALID]), [VALID, OTHER_VALID])
})

test('an id the drop loses nothing for: a non-uuid cannot name a uuid-keyed record', () => {
  // Stated as its own case because it is the reason dropping is safe rather than a bug of its own: the join would have found
  // no row for THE_ROW either way, so the workflow keeps a null property name exactly as it did before the join existed.
  assert.equal(portalJoinableIds([THE_ROW]).length, 0)
  assert.ok(!portalJoinableIds([THE_ROW, VALID]).includes(THE_ROW))
})