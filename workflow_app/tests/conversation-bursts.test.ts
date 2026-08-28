import { test } from 'node:test'
import assert from 'node:assert/strict'

import { groupIntoBursts } from '../../lib/relationship-intel/conversation-bursts'
import type { BurstEvent } from '../../lib/relationship-intel/conversation-bursts'

// ---------------------------------------------------------------------------
// REL-INTEL — conversation-burst projection.
//
// Dense message channels must read as human-sized relationship moments (one
// burst per continuous conversation), NOT thousands of timeline cards. The
// underlying per-event history is never destroyed — this is a pure presentation
// projection. Non-message events (Email / Call / Note / Meeting) pass through
// as single moments unchanged.
// ---------------------------------------------------------------------------

function msg(
  id: string,
  channel: string,
  occurredAt: string,
  direction: BurstEvent['direction'] = 'inbound',
  preview: string | null = null,
): BurstEvent {
  return { id, channel, direction, occurredAt, preview }
}

test('burst: same channel within threshold groups into one burst', () => {
  const bursts = groupIntoBursts([
    msg('m1', 'imessage', '2026-08-27T20:00:00.000Z', 'inbound', 'first'),
    msg('m2', 'imessage', '2026-08-27T20:10:00.000Z', 'outbound'),
    msg('m3', 'imessage', '2026-08-27T20:20:00.000Z', 'inbound', 'latest'),
  ])
  assert.equal(bursts.length, 1)
  assert.equal(bursts[0].count, 3)
  assert.equal(bursts[0].startedAt, '2026-08-27T20:00:00.000Z')
  assert.equal(bursts[0].endedAt, '2026-08-27T20:20:00.000Z')
})

test('burst: gap above threshold splits into a new burst', () => {
  const bursts = groupIntoBursts([
    msg('m1', 'imessage', '2026-08-27T20:00:00.000Z', 'inbound'),
    msg('m2', 'imessage', '2026-08-27T20:10:00.000Z', 'outbound'),
    msg('m3', 'imessage', '2026-08-27T21:00:00.000Z', 'inbound', 'second convo'),
  ])
  // m1/m2 within 10 min -> one burst; m3 is 40 min after m2 -> a new burst.
  assert.equal(bursts.length, 2)
  assert.equal(bursts[0].count, 1) // newest first
  assert.equal(bursts[1].count, 2)
})

test('burst: correct in/out counts and two-way projection', () => {
  const bursts = groupIntoBursts([
    msg('m1', 'imessage', '2026-08-27T20:00:00.000Z', 'inbound'),
    msg('m2', 'imessage', '2026-08-27T20:05:00.000Z', 'outbound'),
    msg('m3', 'imessage', '2026-08-27T20:10:00.000Z', 'outbound'),
  ])
  assert.equal(bursts.length, 1)
  const b = bursts[0]
  assert.equal(b.inboundCount, 1)
  assert.equal(b.outboundCount, 2)
  assert.equal(b.twoWay, true)
  assert.equal(b.direction, 'two-way')

  const inboundOnly = groupIntoBursts([
    msg('m1', 'imessage', '2026-08-27T20:00:00.000Z', 'inbound'),
    msg('m2', 'imessage', '2026-08-27T20:05:00.000Z', 'inbound'),
  ])[0]
  assert.equal(inboundOnly.twoWay, false)
  assert.equal(inboundOnly.direction, 'inbound')
})

test('burst: deterministic latest non-empty preview', () => {
  const bursts = groupIntoBursts([
    msg('m1', 'imessage', '2026-08-27T20:00:00.000Z', 'inbound', 'older cue'),
    msg('m2', 'imessage', '2026-08-27T20:05:00.000Z', 'outbound', null),
    msg('m3', 'imessage', '2026-08-27T20:10:00.000Z', 'inbound', 'NEWEST CUE'),
  ])
  assert.equal(bursts[0].preview, 'NEWEST CUE')
})

test('burst: non-message events pass through as single moments (not destroyed)', () => {
  const bursts = groupIntoBursts([
    msg('note-1', 'note', '2026-08-27T20:00:00.000Z', null, 'manual note'),
    msg('call-1', 'call', '2026-08-27T19:00:00.000Z', 'outbound'),
    msg('m1', 'imessage', '2026-08-27T18:00:00.000Z', 'inbound', 'message'),
  ])
  // Each non-message event is its own single moment; the message stays a burst.
  const byChannel = Object.fromEntries(bursts.map((b) => [b.channel, b]))
  assert.equal(byChannel['note']?.count, 1)
  assert.equal(byChannel['note']?.preview, 'manual note')
  assert.equal(byChannel['call']?.count, 1)
  assert.equal(byChannel['imessage']?.count, 1)
})

test('burst: results are newest-first (newest relationship moment on top)', () => {
  const bursts = groupIntoBursts([
    msg('m1', 'sms', '2026-08-20T10:00:00.000Z', 'inbound', 'old'),
    msg('m2', 'imessage', '2026-08-27T20:00:00.000Z', 'outbound', 'newest'),
    msg('m3', 'imessage', '2026-08-27T19:00:00.000Z', 'inbound', 'older same day'),
  ])
  assert.equal(bursts[0].endedAt, '2026-08-27T20:00:00.000Z')
  // imessage m2+m3 gap 1h -> split; newest burst (20:00) is first.
  assert.ok(bursts[0].endedAt >= bursts[1].endedAt)
})
