// ---------------------------------------------------------------------------
// CATCH-UP — Task tree projection (CATCH-UP three-pane slice).
//
// Proves the TRUE THREE-LEVEL workstream tree derives purely from canonical
// task rows:
//   1. LEVEL 1 = workstream branches in canonical CLIENT/CORE/OPPS/SUPPORT/TECH
//      order, each rendered ONCE as its own branch
//   2. LEVEL 2 = category branches (category IS a tree level, not leaf metadata)
//   3. LEVEL 3 = task leaves that carry ONLY the task title presentation
//   4. blank/unknown workstreams are excluded; blank categories fall back to
//      FALLBACK_CATEGORY so the three-level invariant holds
//   5. categories sort alphabetically; leaves sort by title within a category
//   6. firstCatchUpTaskId returns the first task visible at the initial open
//      state (CLIENT/CORE/SUPPORT open)
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  CATCHUP_WORKSTREAMS,
  FALLBACK_CATEGORY,
  buildCatchUpNavRows,
  buildCatchUpTaskTree,
  firstCatchUpTaskId,
  getCatchUpWorkstreams,
  normalizeWorkstream,
  type CatchUpTaskLeaf,
} from '../../lib/catchup/task-tree'

function leaf(
  partial: Partial<CatchUpTaskLeaf> & Pick<CatchUpTaskLeaf, 'id' | 'title'>,
): CatchUpTaskLeaf {
  return {
    workstream: 'CORE',
    category: null,
    ...partial,
  }
}

test('task-tree: workstream is one branch; category is a real level, not leaf metadata', () => {
  const tree = buildCatchUpTaskTree([
    leaf({ id: 'a', title: 'Call Ana Rivera', workstream: 'CLIENT', category: 'FOLLOWUP' }),
    leaf({ id: 'b', title: 'Onboard accountant', workstream: 'CORE', category: 'ACCOUNTING' }),
    leaf({ id: 'c', title: 'Get business cards', workstream: 'CORE', category: 'MANAGEMENT' }),
  ])

  assert.deepEqual(tree.map((p) => p.id), ['CLIENT', 'CORE'])

  // CLIENT has ONE category branch (FOLLOWUP) whose child is the task leaf.
  const client = tree.find((p) => p.id === 'CLIENT')!
  assert.equal(client.children.length, 1)
  assert.equal(client.children[0].name, 'FOLLOWUP')
  assert.equal(client.children[0].children.length, 1)
  assert.equal(client.children[0].children[0].title, 'Call Ana Rivera')

  // CORE has TWO category branches, each with a single task leaf.
  const core = tree.find((p) => p.id === 'CORE')!
  assert.deepEqual(
    core.children.map((c) => c.name),
    ['ACCOUNTING', 'MANAGEMENT'],
  )
  assert.equal(core.children[0].children[0].title, 'Onboard accountant')
  assert.equal(core.children[1].children[0].title, 'Get business cards')
})

test('task-tree: canonical workstream order; categories alphabetical', () => {
  const tree = buildCatchUpTaskTree([
    leaf({ id: 't', title: 'Harden production environment', workstream: 'TECH', category: 'INFRASTRUCTURE' }),
    leaf({ id: 'f', title: 'Finish Forms', workstream: 'TECH', category: 'NEW_TECH' }),
    leaf({ id: 'm', title: 'DR photography package', workstream: 'CLIENT', category: 'MEDIA' }),
    leaf({ id: 'c', title: 'Listing Agreement — Greece', workstream: 'CLIENT', category: 'CONTRACTS' }),
  ])

  assert.deepEqual(tree.map((p) => p.id), ['CLIENT', 'TECH'])

  // CLIENT: CONTRACTS < MEDIA (alphabetical category branches).
  assert.deepEqual(
    tree[0].children.map((c) => c.name),
    ['CONTRACTS', 'MEDIA'],
  )
  // TECH: INFRASTRUCTURE < NEW_TECH.
  assert.deepEqual(
    tree[1].children.map((c) => c.name),
    ['INFRASTRUCTURE', 'NEW_TECH'],
  )
})

test('task-tree: tasks sort by title within a category branch', () => {
  const tree = buildCatchUpTaskTree([
    leaf({ id: 'z', title: 'Zebra listing', workstream: 'CLIENT', category: 'CONTRACTS' }),
    leaf({ id: 'a', title: 'Alpha listing', workstream: 'CLIENT', category: 'CONTRACTS' }),
  ])
  const contracts = tree[0].children[0]
  assert.equal(contracts.name, 'CONTRACTS')
  assert.deepEqual(
    contracts.children.map((t) => t.title),
    ['Alpha listing', 'Zebra listing'],
  )
})

test('task-tree: full fixture set yields five canonical branches with category sub-branches', () => {
  const fixtures: CatchUpTaskLeaf[] = [
    { id: '1', title: 'Call Ana Rivera', workstream: 'CLIENT', category: 'FOLLOWUP' },
    { id: '2', title: 'Enter DR property data', workstream: 'CLIENT', category: 'ONBOARDING' },
    { id: '3', title: 'Listing Agreement — Greece', workstream: 'CLIENT', category: 'CONTRACTS' },
    { id: '4', title: 'DR photography package', workstream: 'CLIENT', category: 'MEDIA' },
    { id: '5', title: 'Onboard accountant', workstream: 'CORE', category: 'ACCOUNTING' },
    { id: '6', title: 'Get PR Tax EIN', workstream: 'CORE', category: 'LEGAL' },
    { id: '7', title: 'Get business cards', workstream: 'CORE', category: 'MANAGEMENT' },
    { id: '8', title: 'Reconcile listing intake data', workstream: 'OPPS', category: 'DATA_ENTRY' },
    { id: '9', title: 'Verify operating services', workstream: 'SUPPORT', category: 'LIGHTS_ON' },
    { id: '10', title: 'Verify backup cycle', workstream: 'SUPPORT', category: 'BACKUPS' },
    { id: '11', title: 'Finish Forms', workstream: 'TECH', category: 'NEW_TECH' },
    { id: '12', title: 'Harden production environment', workstream: 'TECH', category: 'INFRASTRUCTURE' },
  ]

  const tree = buildCatchUpTaskTree(fixtures)
  assert.deepEqual(tree.map((p) => p.id), [...CATCHUP_WORKSTREAMS])

  // Every workstream has at least one category branch, each with task leaves.
  for (const ws of tree) {
    assert.ok(ws.children.length >= 1)
    for (const cat of ws.children) {
      assert.equal(typeof cat.name, 'string')
      assert.ok(cat.children.length >= 1)
      for (const task of cat.children) {
        assert.equal(typeof task.id, 'string')
        assert.equal(typeof task.title, 'string')
      }
    }
  }

  // TECH branches alphabetically: INFRASTRUCTURE, then NEW_TECH.
  const tech = tree.find((p) => p.id === 'TECH')!
  assert.deepEqual(
    tech.children.map((c) => c.name),
    ['INFRASTRUCTURE', 'NEW_TECH'],
  )
})

test('task-tree: blank category falls back to FALLBACK_CATEGORY to keep three levels', () => {
  const tree = buildCatchUpTaskTree([
    leaf({ id: 'a', title: 'No category task', workstream: 'CLIENT', category: null }),
    leaf({ id: 'b', title: 'Blank category task', workstream: 'CLIENT', category: '' }),
    leaf({ id: 'c', title: 'Real category task', workstream: 'CLIENT', category: 'MARKETING' }),
  ])
  const client = tree.find((p) => p.id === 'CLIENT')!
  assert.deepEqual(
    client.children.map((c) => c.name),
    [FALLBACK_CATEGORY, 'MARKETING'],
  )
  const fallback = client.children.find((c) => c.name === FALLBACK_CATEGORY)!
  assert.equal(fallback.children.length, 2)
})

test('task-tree: blank/unknown workstream rows are excluded', () => {
  const tree = buildCatchUpTaskTree([
    leaf({ id: 'a', title: 'Has workstream', workstream: 'CLIENT', category: 'FOLLOWUP' }),
    leaf({ id: 'b', title: 'Blank workstream', workstream: '' }),
    leaf({ id: 'c', title: 'Null workstream', workstream: null }),
  ])
  assert.deepEqual(tree.map((p) => p.id), ['CLIENT'])
  assert.equal(tree[0].children.length, 1)
})


test('task-tree: unrecognized workstreams sort after the canonical ladder', () => {
  const tree = buildCatchUpTaskTree([
    leaf({ id: 'a', title: 'Canonical', workstream: 'TECH' }),
    leaf({ id: 'z', title: 'Other', workstream: 'ZZZ' }),
    leaf({ id: 'b', title: 'Alpha extra', workstream: 'AAA' }),
  ])
  assert.deepEqual(tree.map((p) => p.id), ['TECH', 'AAA', 'ZZZ'])
})

test('task-tree: normalizeWorkstream is case/whitespace tolerant', () => {
  assert.equal(normalizeWorkstream(' client '), 'CLIENT')
  assert.equal(normalizeWorkstream('core'), 'CORE')
  assert.equal(normalizeWorkstream(''), '')
  assert.equal(normalizeWorkstream(null), '')
  assert.equal(normalizeWorkstream(undefined), '')
})

test('task-tree: firstCatchUpTaskId returns the first task of the first workstream', () => {
  const tasks: CatchUpTaskLeaf[] = [
    leaf({ id: 'a', title: 'Call Ana Rivera', workstream: 'CLIENT', category: 'FOLLOWUP' }),
    leaf({ id: 'b', title: 'Onboard accountant', workstream: 'CORE', category: 'ACCOUNTING' }),
    leaf({ id: 'c', title: 'Finish Forms', workstream: 'TECH', category: 'NEW_TECH' }),
  ]
  assert.equal(firstCatchUpTaskId(tasks), 'a')
})

test('task-tree: firstCatchUpTaskId returns the first task even when only a later workstream has tasks', () => {
  const tasks: CatchUpTaskLeaf[] = [
    leaf({ id: 't', title: 'Only TECH task', workstream: 'TECH', category: 'NEW_TECH' }),
  ]
  assert.equal(firstCatchUpTaskId(tasks), 't')
})

test('task-tree: firstCatchUpTaskId returns null when there are no tasks', () => {
  assert.equal(firstCatchUpTaskId([]), null)
})

test('nav: getCatchUpWorkstreams returns workstreams that have tasks, in canonical order', () => {
  const tasks: CatchUpTaskLeaf[] = [
    leaf({ id: '1', title: 'A', workstream: 'TECH', category: 'INFRASTRUCTURE' }),
    leaf({ id: '2', title: 'B', workstream: 'CLIENT', category: 'CONTRACTS' }),
    leaf({ id: '3', title: 'C', workstream: 'SUPPORT', category: 'SYSTEMS' }),
  ]
  assert.deepEqual(getCatchUpWorkstreams(tasks), ['CLIENT', 'SUPPORT', 'TECH'])
})

test('nav: buildCatchUpNavRows yields category headers then flat task rows for a workstream', () => {
  const tasks: CatchUpTaskLeaf[] = [
    leaf({ id: '1', title: 'Zebra', workstream: 'CLIENT', category: 'MEDIA' }),
    leaf({ id: '2', title: 'Alpha', workstream: 'CLIENT', category: 'CONTRACTS' }),
    leaf({ id: '3', title: 'Beta', workstream: 'CLIENT', category: 'CONTRACTS' }),
    leaf({ id: '4', title: 'Other ws', workstream: 'CORE', category: 'LEGAL' }),
  ]
  const rows = buildCatchUpNavRows(tasks, 'CLIENT')
  // Categories alphabetical; tasks sorted by title within each category.
  assert.deepEqual(
    rows.map((r) => (r.kind === 'category' ? `#${r.name}` : r.title)),
    ['#CONTRACTS', 'Alpha', 'Beta', '#MEDIA', 'Zebra'],
  )
})

test('nav: buildCatchUpNavRows is empty for an unknown workstream', () => {
  const tasks: CatchUpTaskLeaf[] = [
    leaf({ id: '1', title: 'A', workstream: 'CLIENT', category: 'CONTRACTS' }),
  ]
  assert.deepEqual(buildCatchUpNavRows(tasks, 'NOPE'), [])
})

