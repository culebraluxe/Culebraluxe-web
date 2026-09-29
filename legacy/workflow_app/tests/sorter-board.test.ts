import assert from 'node:assert/strict'
import test from 'node:test'

import { buildSorterCards } from '@/lib/sorter-board'

// ---------------------------------------------------------------------------
// ONE STORY, ONE COLUMN — locked here because this invariant has been the root cause
// of two separate wrong-card bugs on the Cockpit's sorter:
//
//   1. a bench story also drawn in OPEN (the bench is an intent row, so the status
//      stayed `In Progress`) - two cards with the same id, and the vendor's drop
//      handler finds cards by id, so moving one moved the other.
//   2. `Ready` (handed to ENGINE RUN Q) maps to the OPEN lifecycle, so the ONE move
//      that starts machine work drew the story in OPEN and ENGINE RUN Q at once.
//
// The assignment now happens in one pass with one `claimed` set, so a duplicate is not
// filtered out afterwards - it cannot be created. These tests hold that down.
// ---------------------------------------------------------------------------

const story = (id: string, status: string) => ({
  id,
  title: `${id} title`,
  status,
  priority: 'MEDIUM',
  completion: 0,
})

const panels = (open: ReturnType<typeof story>[], backlog: ReturnType<typeof story>[] = []) => ({
  open: { groups: [{ stories: open }] },
  backlog: { groups: [{ stories: backlog }] },
  'next-version': { groups: [{ stories: [] }] },
  closed: { groups: [{ stories: [] }] },
})

const idsOf = (cards: ReturnType<typeof buildSorterCards>, column: string) =>
  cards.filter((c) => c.column === column).map((c) => c.id.split('#')[0])

test('a story handed to the engine is in ENGINE RUN Q and NOT in OPEN', () => {
  // `Ready` maps to the OPEN lifecycle, so the cockpit hands it to us inside the open panel.
  const cards = buildSorterCards({
    panels: panels([story('ENG-1', 'Ready')]),
    queuedCards: [{ storyId: 'ENG-1', title: 'ENG-1 title', state: 'Ready' }],
  })
  assert.deepEqual(idsOf(cards, 'engine'), ['ENG-1'])
  assert.deepEqual(idsOf(cards, 'open'), [])
})

test('a handed-over story with no queue row still lands in ENGINE RUN Q, not OPEN', () => {
  // The engine may have no `agent_work_item` for it any more (or yet). The story is still handed
  // over; drawing it in OPEN is what put the handoff in two columns.
  const cards = buildSorterCards({ panels: panels([story('ENG-2', 'Ready')]) })
  assert.deepEqual(idsOf(cards, 'engine'), ['ENG-2'])
  assert.deepEqual(idsOf(cards, 'open'), [])
})

test('a bench story is not also drawn in OPEN', () => {
  const cards = buildSorterCards({
    // The bench is an intent row: the story keeps `In Progress` and is in the open panel too.
    panels: panels([story('OPEN-1', 'In Progress')]),
    activeWork: [story('OPEN-1', 'In Progress')],
  })
  assert.deepEqual(idsOf(cards, 'bench'), ['OPEN-1'])
  assert.deepEqual(idsOf(cards, 'open'), [])
})

test('a staged story that is also on the bench is drawn once, in ENGINE BATCH', () => {
  const cards = buildSorterCards({
    panels: panels([], [story('B-1', 'Batched')]),
    batchStories: [story('B-1', 'Batched')],
    activeWork: [story('B-1', 'Batched')],
  })
  assert.deepEqual(idsOf(cards, 'batch'), ['B-1'])
  assert.deepEqual(idsOf(cards, 'bench'), [])
  assert.deepEqual(idsOf(cards, 'backlog'), [])
})

test('a bench story the engine has taken is drawn once, in ENGINE RUN Q', () => {
  const cards = buildSorterCards({
    panels: panels([story('H-1', 'In Progress')]),
    activeWork: [story('H-1', 'In Progress')],
    queuedCards: [{ storyId: 'H-1', title: 'H-1 title', state: 'Claimed' }],
  })
  assert.deepEqual(idsOf(cards, 'engine'), ['H-1'])
  assert.deepEqual(idsOf(cards, 'bench'), [])
  assert.deepEqual(idsOf(cards, 'open'), [])
})

test('no story id is ever drawn twice across the whole board', () => {
  // A deliberately adversarial mixture: overlap attempts on every axis at once.
  const cards = buildSorterCards({
    panels: panels(
      [
        story('OPEN-1', 'In Progress'),
        story('OPEN-2', 'In Progress'),
        story('READY-1', 'Ready'),
        story('DUP-1', 'In Progress'),
      ],
      [story('BACK-1', 'Planned'), story('DUP-1', 'Batched')],
    ),
    activeWork: [story('DUP-1', 'In Progress'), story('OPEN-2', 'In Progress')],
    batchStories: [story('DUP-1', 'Batched')],
    engineRuns: [
      // Two ATTEMPTS of one story: the sorter draws one card, the engine panel shows both.
      {
        id: 'RUN-1#1',
        storyId: 'RUN-1',
        title: 'RUN-1 title',
        status: 'running',
        priority: 'MEDIUM',
        completion: 0,
        queue: 'running',
      },
      {
        id: 'RUN-1#2',
        storyId: 'RUN-1',
        title: 'RUN-1 title',
        status: 'running',
        priority: 'MEDIUM',
        completion: 0,
        queue: 'running',
      },
      // Finished work belongs to the engine panel's RESULTS lane, never to the sorter.
      {
        id: 'DONE-1#1',
        storyId: 'DONE-1',
        title: 'DONE-1 title',
        status: 'completed',
        priority: 'MEDIUM',
        completion: 100,
        queue: 'results',
      },
    ],
    queuedCards: [
      { storyId: 'READY-1', title: 'READY-1 title', state: 'Ready' },
      { storyId: 'RUN-1', title: 'RUN-1 title', state: 'Claimed' },
    ],
  })

  const seen = new Map<string, string>()
  for (const c of cards) {
    const key = c.id.split('#')[0]
    assert.equal(seen.has(key), false, `${key} drawn twice: ${seen.get(key)} and ${c.column}`)
    seen.set(key, c.column)
  }
  // And the placement is the one the board promises.
  assert.equal(seen.get('RUN-1'), 'engine')
  assert.equal(seen.get('READY-1'), 'engine')
  assert.equal(seen.get('DUP-1'), 'batch')
  assert.equal(seen.get('OPEN-2'), 'bench')
  assert.equal(seen.get('OPEN-1'), 'open')
  assert.equal(seen.get('BACK-1'), 'backlog')
  assert.equal(seen.has('DONE-1'), false, 'finished work must not appear on the sorter')
})

// ---------------------------------------------------------------------------
// THE KIND RIDES THE CARD (Phase 1 follow-up). The kind is chosen on the batch MEMBER
// (`forge_batch_item.kind`, migration 179), not on the story, so the sorter must pass it
// through rather than look it up. What matters to the board is the difference between "read as
// fix" and "not read": a card with no kind must render no chip, never a default the dispatch
// never wrote.
// ---------------------------------------------------------------------------

test('a staged card carries its kind, and an unread kind stays null rather than guessing', () => {
  const cards = buildSorterCards({
    panels: panels([], [story('PLAIN-1', 'Batched')]),
    batchStories: [
      {
        id: 'KIND-1',
        title: 'KIND-1 title',
        status: 'Batched',
        priority: 'MEDIUM',
        completion: 0,
        kind: 'fix',
      },
      {
        id: 'PLAIN-1',
        title: 'PLAIN-1 title',
        status: 'Batched',
        priority: 'MEDIUM',
        completion: 0,
      },
    ],
  })

  const byId = new Map(cards.map((c) => [c.id, c]))
  assert.equal(byId.get('KIND-1')?.column, 'batch')
  assert.equal(byId.get('KIND-1')?.kind, 'fix')
  // The absence is the assertion: no kind was read for this story, so the card must not claim one.
  assert.equal(byId.get('PLAIN-1')?.kind, null)
})

test('a kind survives the other columns too, so one card shape serves the whole board', () => {
  const cards = buildSorterCards({
    panels: panels([], []),
    activeWork: [{ ...story('BENCH-1', 'In Progress'), kind: 'judgment' }],
  })
  const bench = cards.find((c) => c.id === 'BENCH-1')
  assert.equal(bench?.column, 'bench')
  assert.equal(bench?.kind, 'judgment')
})

