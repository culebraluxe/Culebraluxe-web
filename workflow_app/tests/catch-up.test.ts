import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFileSync } from 'node:fs'

import {
  deriveAttention,
  type CatchUpPersonFacts,
} from '../../lib/catchup/rules'
import { buildCatchUpQueue, toFacts } from '../../lib/catchup/queue'
import { normalizeLeadInput } from '../../lib/catchup/lead-intake'
import {
  normalizeCalendarEvent,
  weekBuckets,
  bucketForEvent,
} from '../../lib/catchup/calendar-adapter'
import { getCatchUpEligiblePage } from '../../db/catch-up'
import { createWebsiteLead } from '../../db/catchup-lead'

// ---------------------------------------------------------------------------
// CATCH-UP — focused V1 proofs: attention rules, queue assembly, paging
// (ENG-34), calendar adapter, website lead intake, and CommandStatusBand reuse.
// ---------------------------------------------------------------------------

const DAY = 24 * 60 * 60 * 1000
const NOW = Date.now()

function facts(partial: Partial<CatchUpPersonFacts>): CatchUpPersonFacts {
  return {
    id: 'p1',
    displayName: 'Jane Doe',
    role: 'buyer',
    status: 'new',
    email: 'jane@example.com',
    phone: null,
    createdAt: new Date(NOW - 30 * DAY).toISOString(),
    lastMeaningfulContactAt: null,
    lastInboundAt: null,
    lastOutboundAt: null,
    activeDealStage: null,
    activeDealProperty: null,
    nextEventAt: null,
    nextEventLabel: null,
    ...partial,
  }
}

// --- attention rules ---
test('catch-up rules: new website lead with no response', () => {
  const item = deriveAttention(
    facts({ createdAt: new Date(NOW - DAY).toISOString(), lastOutboundAt: null }),
    NOW,
  )
  assert.ok(item)
  assert.equal(item!.reasonCode, 'new_lead')
  assert.match(item!.reasonLabel, /New/)
})

test('catch-up rules: recent inbound needs a response', () => {
  const item = deriveAttention(
    facts({
      createdAt: new Date(NOW - 30 * DAY).toISOString(),
      lastInboundAt: new Date(NOW - DAY).toISOString(),
      lastOutboundAt: null,
    }),
    NOW,
  )
  assert.ok(item)
  assert.equal(item!.reasonCode, 'needs_response')
  assert.match(item!.reasonLabel, /Inbound/)
})

test('catch-up rules: upcoming calendar event drives attention', () => {
  const item = deriveAttention(
    facts({
      createdAt: new Date(NOW - 30 * DAY).toISOString(),
      nextEventAt: new Date(NOW + DAY).toISOString(),
      nextEventLabel: 'Showing tomorrow at 10:00 AM',
    }),
    NOW,
  )
  assert.ok(item)
  assert.equal(item!.reasonCode, 'calendar_attention')
  assert.match(item!.reasonLabel, /Showing tomorrow/)
})

test('catch-up rules: active deal participant quiet 5+ days', () => {
  const item = deriveAttention(
    facts({
      createdAt: new Date(NOW - 30 * DAY).toISOString(),
      activeDealStage: 'under_contract',
      activeDealProperty: 'Villa Mar',
      lastMeaningfulContactAt: new Date(NOW - 6 * DAY).toISOString(),
    }),
    NOW,
  )
  assert.ok(item)
  assert.equal(item!.reasonCode, 'active_deal_attention')
  assert.match(item!.reasonLabel, /Closing/)
})

test('catch-up rules: stale tickle when quiet 10+ days', () => {
  const item = deriveAttention(
    facts({
      createdAt: new Date(NOW - 30 * DAY).toISOString(),
      lastMeaningfulContactAt: new Date(NOW - 12 * DAY).toISOString(),
    }),
    NOW,
  )
  assert.ok(item)
  assert.equal(item!.reasonCode, 'stale_tickle')
  assert.match(item!.reasonLabel, /No meaningful contact/)
})

test('catch-up rules: recent meaningful contact produces no reason', () => {
  const item = deriveAttention(
    facts({
      createdAt: new Date(NOW - 30 * DAY).toISOString(),
      lastMeaningfulContactAt: new Date(NOW - DAY).toISOString(),
      lastInboundAt: null,
      lastOutboundAt: new Date(NOW - DAY).toISOString(),
      activeDealStage: null,
      nextEventAt: null,
    }),
    NOW,
  )
  assert.equal(item, null)
})

// --- queue assembly ---
test('catch-up queue keeps only people with a reason, urgency first', () => {
  const rows = [
    {
      personId: 'p-stale',
      displayName: 'Avery',
      role: 'buyer',
      status: 'active',
      email: null,
      phone: null,
      createdAt: new Date(NOW - 30 * DAY).toISOString(),
      lastMeaningfulContactAt: new Date(NOW - 20 * DAY).toISOString(),
      lastInboundAt: null,
      lastOutboundAt: null,
      activeDealStage: null,
      activeDealProperty: null,
      nextEventAt: null,
      nextEventLabel: null,
    },
    {
      personId: 'p-recent',
      displayName: 'Recent',
      role: 'buyer',
      status: 'new',
      email: 'r@example.com',
      phone: null,
      createdAt: new Date(NOW - 30 * DAY).toISOString(),
      lastMeaningfulContactAt: new Date(NOW - DAY).toISOString(),
      lastInboundAt: null,
      lastOutboundAt: new Date(NOW - DAY).toISOString(),
      activeDealStage: null,
      activeDealProperty: null,
      nextEventAt: null,
      nextEventLabel: null,
    },
  ]
  const queue = buildCatchUpQueue(rows)
  assert.equal(queue.length, 1)
  assert.equal(queue[0].personId, 'p-stale')
})

// --- paging (ENG-34) ---
type Row = Record<string, any>
function fakeExecute(pageRows: Row[], factsRows: Row[], total: number) {
  const calls: Array<{ text: string; params: any[] }> = []
  const fn: any = (strings: TemplateStringsArray, ...params: any[]) => {
    const text = strings.join('__').replace(/\s+/g, ' ').trim().toLowerCase()
    calls.push({ text, params })
    if (text.includes('count(*)')) return [{ total }]
    if (text.includes(' any (')) return factsRows
    return pageRows
  }
  fn.calls = calls
  return fn
}

test('catch-up paging: bounded page + COUNT + facts (no full hydration)', async () => {
  const execute = fakeExecute(
    [{ person_id: 'p1', display_name: 'Jane', role: 'buyer', status: 'new', created_at: '2026-08-24T00:00:00Z' }],
    [{ person_id: 'p1', email: 'jane@example.com', phone: null, last_meaningful_contact_at: null, last_inbound_at: null, last_outbound_at: null, active_deal_stage: null, active_deal_property: null, next_event_at: null, next_event_label: null }],
    137,
  )
  const result = await getCatchUpEligiblePage(
    { search: 'jane', page: 2, pageSize: 50 },
    execute as never,
  )
  assert.equal(result.total, 137)
  assert.equal(result.page, 2)
  assert.equal(result.pageSize, 50)
  assert.equal(result.rows.length, 1)
  assert.equal(result.rows[0].email, 'jane@example.com')
  assert.ok(execute.calls.some((c) => c.text.includes('count(*)')), 'COUNT query present')
  assert.ok(execute.calls.some((c) => c.params.includes(50)), 'pageSize reaches LIMIT')
  assert.ok(execute.calls.some((c) => c.text.includes(' any (')), 'facts read is bounded by id list')
})


test('catch-up read model is canonical, not ODS staging (source guard)', () => {
  const src = readFileSync(new URL('../../db/catch-up.ts', import.meta.url), 'utf8')
  assert.ok(/from person p/.test(src), 'eligible population reads canonical person')
  assert.ok(/from interaction/.test(src), 'contact timing reads canonical interaction')
  assert.ok(/from showing/.test(src), 'calendar attention reads canonical showing')
  assert.ok(!/l_person/.test(src), 'no ODS staging as the product read model')
  assert.ok(!/refresh materialized view/i.test(src), 'no GET-time MV refresh')
})

// --- calendar adapter ---
test('catch-up calendar: normalize + week bucketing', () => {
  const e = normalizeCalendarEvent({
    id: 'showing:1',
    title: 'Showing',
    startAt: '2026-08-27T14:00:00.000Z',
    endAt: null,
    allDay: false,
    personId: 'p1',
    personName: 'Jane',
    propertyName: 'Villa',
    kind: 'showing',
    source: 'canonical:showing',
  })
  assert.equal(e.kind, 'showing')
  const buckets = weekBuckets(new Date('2026-08-27T00:00:00.000Z'))
  assert.equal(buckets.length, 7)
  assert.ok(bucketForEvent(e, buckets), 'event lands in the week')
})

// --- CommandStatusBand reuse ---
test('catch-up reuses CommandStatusBand (balanced 50/50, no local copy)', () => {
  const src = readFileSync(new URL('../../components/portal/catch-up.tsx', import.meta.url), 'utf8')
  assert.ok(/CommandStatusBand/.test(src), 'imports the shared band')
  assert.ok(/ratio="command-6040"/.test(src), 'uses the shared 60/40 ratio preset aligned to the Calendar seam')
  assert.ok(!/RATIO_GRID/.test(src), 'no local ratio system')
  // The band's desktop gap must line up with the TREE | DETAIL | CALENDAR row.
  assert.ok(/lg:gap-4/.test(src), 'workspace row shares the band gap for exact seam alignment')
  assert.ok(/grid-cols-\[minmax/.test(src), 'three-pane desktop grid (TREE | DETAIL | CALENDAR)')
  assert.ok(!/lg:grid-cols-2/.test(src), 'no longer a two-column layout')
})

// --- three-pane composition (navigator + FullCalendar) ---
test('catch-up screen composes flat navigator + FullCalendar and preserves Option A source', () => {
  const src = readFileSync(new URL('../../components/portal/catch-up.tsx', import.meta.url), 'utf8')
  assert.ok(/CatchUpWorkQueue/.test(src), 'left pane is the flat work-queue navigator')
  assert.ok(/CatchUpTaskDetail/.test(src), 'middle pane is the task workspace')
  assert.ok(/FullCalendarCandidate/.test(src), 'right pane is FullCalendar (Option B)')
  assert.ok(!/CatchUpTaskTree/.test(src), 'the active screen no longer uses the Arborist tree')
  assert.ok(!/CatchUpCalendarEvaluation/.test(src), 'the A/B evaluation harness is not rendered')
  // Option A (@ilamy/calendar) source is preserved in the repo, not rendered.
  const optionA = readFileSync(
    new URL('../../components/portal/catch-up-calendar.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/IlamyCalendar/.test(optionA), 'Option A source preserved')
})

// --- website lead intake ---
test('lead intake: accepts Name + Email', () => {
  const r = normalizeLeadInput({ name: 'Jane Doe', email: 'jane@example.com' })
  assert.equal(r.ok, true)
  if (r.ok) assert.equal(r.value.email, 'jane@example.com')
})

test('lead intake: accepts Name + Phone', () => {
  const r = normalizeLeadInput({ name: 'Jane Doe', phone: '+1 787 555 0134' })
  assert.equal(r.ok, true)
  if (r.ok) assert.equal(r.value.phone, '+17875550134')
})

test('lead intake: rejects missing name and missing contact', () => {
  const r = normalizeLeadInput({ name: '', email: '' })
  assert.equal(r.ok, false)
  if (!r.ok) {
    assert.ok(r.errors.name)
    assert.ok(r.errors.email)
  }
})

test('lead intake: rejects invalid email', () => {
  const r = normalizeLeadInput({ name: 'Jane', email: 'not-an-email' })
  assert.equal(r.ok, false)
})

test('catch-up calendar uses a real library and preserves the normalized boundary (source guard)', () => {
  const src = readFileSync(
    new URL('../../components/portal/catch-up-calendar.tsx', import.meta.url),
    'utf8',
  )
  assert.ok(/@ilamy\/calendar/.test(src), 'uses the @ilamy/calendar engine')
  assert.ok(/IlamyCalendar/.test(src), 'renders IlamyCalendar')
  assert.ok(/headerComponent/.test(src), 'owns the header via headerComponent')
  assert.ok(/initialView="month"/.test(src), 'defaults to Month view')
  assert.ok(/disableDragAndDrop/.test(src), 'read-only (no event drag)')
  assert.ok(
    !/from '@\/db'/.test(src),
    'no database imports inside the calendar component',
  )
  assert.ok(
    !/EventKit|CalDAV|mac-observer/.test(src),
    'library/component never touches Apple/EventKit/provider internals',
  )
})


// --- canonical lead write ---
const INTERACTION_ROW = {
  id: 'i1',
  person_id: 'p-new',
  property_id: null,
  deal_id: null,
  channel: 'website',
  event_type: 'lead_inquiry',
  direction: 'inbound',
  occurred_at: new Date().toISOString(),
  title: 'Website inquiry',
  summary: null,
  duration_seconds: null,
  source_system: 'website',
  source_external_id: 'sub-1',
  source_metadata: {},
  created_at: new Date().toISOString(),
}

function leadFake(identityRows: { person_id: string }[], interactionRow: Row | null) {
  const calls: string[] = []
  const fn: any = (strings: TemplateStringsArray) => {
    const text = strings.join('__').replace(/\s+/g, ' ').trim().toLowerCase()
    calls.push(text)
    if (text.includes('from person_identity') && text.includes('identity_type')) {
      return Promise.resolve(identityRows)
    }
    if (text.includes('insert into person') && !text.includes('person_identity')) {
      return Promise.resolve([])
    }
    if (text.includes('insert into person_identity')) {
      return Promise.resolve([])
    }
    if (text.includes('insert into interaction')) {
      return Promise.resolve(interactionRow ? [interactionRow] : [])
    }
    if (text.includes('from interaction')) {
      return Promise.resolve([])
    }
    return Promise.resolve([])
  }
  fn.calls = calls
  return fn
}

test('lead write: creates a canonical person + interaction when identity unowned', async () => {
  const execute = leadFake([], INTERACTION_ROW)
  const result = await createWebsiteLead(
    { name: 'Jane Doe', email: 'jane@example.com' },
    execute as never,
  )
  assert.equal(result.status, 'created')
  assert.ok(result.personId)
  assert.ok(execute.calls.some((c: string) => c.includes('insert into person')), 'canonical person created')
  assert.ok(execute.calls.some((c: string) => c.includes('insert into interaction')), 'provenance interaction written')
})

test('lead write: resolves an existing identity, never merges', async () => {
  const execute = leadFake([{ person_id: 'p-existing' }], INTERACTION_ROW)
  const result = await createWebsiteLead(
    { name: 'Jane Doe', email: 'jane@example.com' },
    execute as never,
  )
  assert.equal(result.status, 'resolved')
  assert.equal(result.personId, 'p-existing')
  assert.ok(!execute.calls.some((c: string) => c.includes('insert into person') && !c.includes('person_identity')), 'no duplicate person')
})

test('lead write: ambiguous identity never silently merges', async () => {
  const execute = leadFake([{ person_id: 'p1' }, { person_id: 'p2' }], INTERACTION_ROW)
  const result = await createWebsiteLead(
    { name: 'Jane Doe', phone: '+17875550134' },
    execute as never,
  )
  assert.equal(result.status, 'resolution_required')
  assert.equal(result.personId, null)
})

// --- toFacts mapping ---
test('catch-up toFacts maps the read-model row to rule facts', () => {
  const f = toFacts({
    personId: 'p1',
    displayName: 'Jane',
    role: 'buyer',
    status: 'new',
    email: 'jane@example.com',
    phone: null,
    createdAt: new Date().toISOString(),
    lastMeaningfulContactAt: null,
    lastInboundAt: null,
    lastOutboundAt: null,
    activeDealStage: null,
    activeDealProperty: null,
    nextEventAt: null,
    nextEventLabel: null,
  })
  assert.equal(f.id, 'p1')
  assert.equal(f.displayName, 'Jane')
})

