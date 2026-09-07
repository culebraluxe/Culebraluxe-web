// ---------------------------------------------------------------------------
// TESTV2 UI tier — pure view-model projection for the client CRM rail.
//
// Glass-box proof for the MVI separation: this spec imports ONLY the pure
// channel→slot projection (`channel-projection.ts`). No controller, no React,
// no source adapter, no DB. It asserts the SLOT_DEFINITIONS mapping logic that
// turns one source-grain ClientRelationshipChannel row per Person into the six
// canonical relationship slots the CRM screen renders.
//
// Note: tsx does not typecheck; the fixtures are intentionally thin but faithful
// to the fields the projection actually reads (source, channel, counts, dates).
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import { projectClientLensChannels } from '../ui/client-lens/channel-projection'

type Chan = {
  personId: string
  source: string
  channel: string
  firstObservedAt?: string | null
  lastContactAt?: string | null
  lastInboundAt?: string | null
  lastOutboundAt?: string | null
  inboundCount?: number
  outboundCount?: number
  totalCount?: number
  twoWay?: boolean
  lastContext?: string | null
  lastContextAt?: string | null
  lastContextDirection?: 'inbound' | 'outbound' | null
}

function chan(personId: string, source: string, channel: string, over: Partial<Chan> = {}): Chan {
  return {
    personId,
    source,
    channel,
    firstObservedAt: null,
    lastContactAt: null,
    lastInboundAt: null,
    lastOutboundAt: null,
    inboundCount: 0,
    outboundCount: 0,
    totalCount: 0,
    twoWay: false,
    lastContext: null,
    lastContextAt: null,
    lastContextDirection: null,
    ...over,
  }
}

const P = 'person-1'

test('empty channel set projects six canonical slots, all disconnected', () => {
  const slots = projectClientLensChannels([])
  assert.equal(slots.length, 6)
  assert.deepEqual(
    slots.map((s) => s.slot),
    ['phone', 'imessage', 'whatsapp', 'gmail', 'facetime', 'calendar'],
  )
  assert.ok(slots.every((s) => s.connected === false))
  assert.ok(slots.every((s) => s.source === null && s.channel === null))
})

test('one channel per canonical source maps to the expected slot with its counters', () => {
  const rows = [
    chan(P, 'call_log', 'call', {
      inboundCount: 3,
      outboundCount: 1,
      totalCount: 4,
      twoWay: true,
      lastContext: 'booked a showing',
      lastContextDirection: 'inbound',
    }),
    chan(P, 'apple_messages', 'imessage', { totalCount: 9, twoWay: true }),
    chan(P, 'whatsapp', 'whatsapp', { totalCount: 2 }),
    chan(P, 'gmail', 'email', { totalCount: 7 }),
    chan(P, 'facetime', 'call', { totalCount: 1 }),
    chan(P, 'eventkit', 'calendar', { totalCount: 5 }),
  ]
  const slots = projectClientLensChannels(rows)
  assert.equal(slots.length, 6)
  const by = Object.fromEntries(slots.map((s) => [s.slot, s]))

  assert.equal(by.phone.slot, 'phone')
  assert.equal(by.phone.connected, true)
  assert.equal(by.phone.source, 'call_log')
  assert.equal(by.phone.inboundCount, 3)
  assert.equal(by.phone.outboundCount, 1)
  assert.equal(by.phone.totalCount, 4)
  assert.equal(by.phone.twoWay, true)
  assert.equal(by.phone.lastContext, 'booked a showing')

  assert.equal(by.imessage.source, 'apple_messages')
  assert.equal(by.whatsapp.source, 'whatsapp')
  assert.equal(by.gmail.source, 'gmail')
  assert.equal(by.facetime.source, 'facetime')
  assert.equal(by.calendar.source, 'eventkit')
  assert.ok(slots.every((s) => s.connected))
})

test('slot classification is exclusive (no double counting) for ambiguous sources', () => {
  // A FaceTime call is a "call" channel but must land in the facetime slot,
  // never the phone slot (phone excludes sources containing 'facetime').
  const slots = projectClientLensChannels([chan(P, 'facetime', 'call', { totalCount: 1 })])
  assert.equal(slots.find((s) => s.slot === 'facetime')?.connected, true)
  assert.equal(slots.find((s) => s.slot === 'phone')?.connected, false)

  // An email row materialized from an Apple Calendar source is calendar
  // evidence, not gmail (gmail requires the gmail/calendar exclusion).
  const cal = projectClientLensChannels([chan(P, 'calendar', 'email', { totalCount: 1 })])
  assert.equal(cal.find((s) => s.slot === 'calendar')?.connected, true)
  assert.equal(cal.find((s) => s.slot === 'gmail')?.connected, false)
})

test('two channels claiming the same slot keep the first source-grain row', () => {
  const rows = [
    chan(P, 'call_log', 'call', { totalCount: 3 }),
    chan(P, 'phone', 'call', { totalCount: 1 }),
  ]
  const slots = projectClientLensChannels(rows)
  const phone = slots.find((s) => s.slot === 'phone')
  assert.equal(phone?.connected, true)
  assert.equal(phone?.source, 'call_log')
  assert.equal(phone?.totalCount, 3)
})
