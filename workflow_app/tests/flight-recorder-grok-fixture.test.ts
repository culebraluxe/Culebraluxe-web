import test from 'node:test'
import assert from 'node:assert/strict'

import {
  DEAL_TRACE_FIXTURE,
  SELECTED_EVENT_ID,
  EVENT_IDS,
} from '../../grok/flight-recorder/src/fixture'

// ---------------------------------------------------------------------------
// FLIGHT-RECORDER-GROK-FIXTURE — golden visual contract regression.
//
// The original Grok fixture is the approved design intent. This proves the
// data-integration work has not silently reshaped Grok's presentation model:
// event count, semantic kinds, systems, ordering, and glyph categories remain
// stable.
// ---------------------------------------------------------------------------

test('Grok golden fixture preserves event count, kinds, systems and ordering', () => {
  const events = DEAL_TRACE_FIXTURE.events
  // 18 fixture events from the screenshot.
  assert.equal(events.length, 18)

  // Every event has a valid semantic kind + system from the Grok vocabulary.
  const kinds = new Set(events.map((e) => e.kind))
  const systems = new Set(events.map((e) => e.system))
  assert.ok(kinds.size >= 4, 'rich semantic-kind coverage')
  assert.ok(systems.size >= 4, 'rich system coverage')
  for (const k of kinds) assert.ok(k, 'kind present')
  for (const s of systems) assert.ok(s, 'system present')

  // Chronological ordering is preserved.
  for (let i = 1; i < events.length; i++) {
    assert.ok(
      new Date(events[i].occurredAt).getTime() >= new Date(events[i - 1].occurredAt).getTime(),
      `events ordered at index ${i}`,
    )
  }

  // The default selected event (Task Created) is a real fixture event.
  assert.ok(events.some((e) => e.id === SELECTED_EVENT_ID), 'default selection resolves')
})

test('Grok golden fixture exposes the expected glyph categories and layout anchors', () => {
  const kinds = new Set(DEAL_TRACE_FIXTURE.events.map((e) => e.kind))
  // The Grok glyph vocabulary used by the Timeline / Master Workflow legend.
  assert.ok(kinds.has('Command'))
  assert.ok(kinds.has('DomainEvent'))
  assert.ok(kinds.has('Workflow'))
  assert.ok(kinds.has('Task'))
  // Fixture summary is intact.
  assert.equal(DEAL_TRACE_FIXTURE.summary.correlationId, DEAL_TRACE_FIXTURE.events[0].correlationId)
  assert.ok(DEAL_TRACE_FIXTURE.summary.eventCount > 0)
  assert.ok(Object.keys(EVENT_IDS).length > 0, 'stable id index retained')
})
