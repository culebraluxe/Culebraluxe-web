import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  appleServiceToChannel,
  mapAppleMessageToInteraction,
  resolveHandlePerson,
} from '../../lib/relationship-intel/apple-message-materializer'
import { APPLE_MESSAGES_SOURCE } from '../../lib/relationship-intel/apple-messages'
import { summarizeRelationshipEvidence } from '../../lib/relationship-intel/relationship-context'
import type { AppleMessagesMessage } from '../../lib/relationship-intel/apple-messages'

// ---------------------------------------------------------------------------
// REL-INTEL — Apple Messages EVENT materialization (the missing final leg).
//
// Covers the pure mapping (channel / direction / replay key / no private
// prose), the authoritative-handle gating (ambiguous/unmatched handles are
// never silently assigned), and the source-specific relationship-memory
// projection (channels[]). Replay/idempotency is additionally proven against
// DEV by the real-data proof script (first run N, second run 0 duplicates).
// ---------------------------------------------------------------------------

const msg = (over: Partial<AppleMessagesMessage>): AppleMessagesMessage => ({
  rowid: 1,
  guid: 'GUID-1',
  chatGuid: null,
  handleId: 1,
  handleValue: '1',
  service: 'iMessage',
  date: null,
  dateISO: '2026-08-25T20:42:00.000Z',
  isFromMe: 0,
  text: null,
  hasAttachments: 0,
  ...over,
})

test('appleServiceToChannel: iMessage vs SMS from Apple service evidence', () => {
  assert.equal(appleServiceToChannel('iMessage'), 'imessage')
  assert.equal(appleServiceToChannel('iMessageBusinessChat'), 'imessage')
  assert.equal(appleServiceToChannel('SMS'), 'sms')
  assert.equal(appleServiceToChannel('SMSReply'), 'sms')
  assert.equal(appleServiceToChannel(null), 'imessage')
})

test('mapAppleMessageToInteraction: inbound -> channel/direction/replay key, no body', () => {
  const input = mapAppleMessageToInteraction(
    msg({ guid: 'GUID-IN-1', service: 'iMessage', isFromMe: 0, text: 'SECRET BODY' }),
    'person-1',
    'local_mac',
  )
  assert.equal(input.personId, 'person-1')
  assert.equal(input.channel, 'imessage')
  assert.equal(input.eventType, 'message')
  assert.equal(input.direction, 'inbound')
  assert.equal(input.occurredAt, '2026-08-25T20:42:00.000Z')
  assert.equal(input.sourceSystem, APPLE_MESSAGES_SOURCE)
  assert.equal(input.sourceExternalId, 'GUID-IN-1') // replay key
  assert.equal(input.title, undefined) // no prose
  assert.equal(input.summary, undefined) // no prose
  const meta = input.sourceMetadata as Record<string, unknown>
  assert.equal(meta.sourceAccount, 'local_mac')
  assert.equal(meta.handleId, 1)
  assert.equal(JSON.stringify(input.sourceMetadata).includes('SECRET BODY'), false)
})

test('mapAppleMessageToInteraction: outbound + sms channel', () => {
  const input = mapAppleMessageToInteraction(
    msg({ guid: 'GUID-OUT-1', service: 'SMS', isFromMe: 1, hasAttachments: 1 }),
    'person-2',
    'local_mac',
  )
  assert.equal(input.channel, 'sms')
  assert.equal(input.direction, 'outbound')
  assert.equal(input.sourceExternalId, 'GUID-OUT-1')
})

test('resolveHandlePerson: only exact_linked with canonical person qualifies', () => {
  const map = new Map([
    ['+17875550134', { reviewState: 'exact_linked', canonicalPersonId: 'person-1' }],
    ['+17875550987', { reviewState: 'ambiguous', canonicalPersonId: null }],
    ['+17875550000', { reviewState: 'unmatched', canonicalPersonId: null }],
    ['+17875551111', { reviewState: 'deferred', canonicalPersonId: null }],
    ['+17875552222', { reviewState: 'exact_linked', canonicalPersonId: null }],
  ])
  assert.deepEqual(resolveHandlePerson(map, '+17875550134'), {
    ok: true,
    canonicalPersonId: 'person-1',
  })
  assert.equal(resolveHandlePerson(map, '+17875550987').ok, false) // ambiguous
  assert.equal(resolveHandlePerson(map, '+17875550000').ok, false) // unmatched
  assert.equal(resolveHandlePerson(map, '+17875551111').ok, false) // deferred
  assert.equal(resolveHandlePerson(map, '+17875552222').ok, false) // no canonical person
  assert.equal(resolveHandlePerson(map, '+999').ok, false) // no evidence
})

test('channels: truthful per-source relationship-memory projection', () => {
  const s = summarizeRelationshipEvidence([
    {
      source: 'apple_messages',
      inboundCount: 3,
      outboundCount: 2,
      lastObservedAt: '2026-08-25T20:42:00.000Z',
      lastInboundAt: '2026-08-25T20:42:00.000Z',
      lastOutboundAt: '2026-08-24T09:00:00.000Z',
      isTwoWay: true,
      hasEmail: false,
      hasPhone: true,
      coverageNote: null,
    },
    {
      source: 'gmail_contacts',
      inboundCount: 5,
      outboundCount: 8,
      lastObservedAt: '2026-08-12T14:14:00.000Z',
      lastInboundAt: '2026-08-12T14:14:00.000Z',
      lastOutboundAt: '2026-08-10T10:00:00.000Z',
      isTwoWay: true,
      hasEmail: true,
      hasPhone: false,
      coverageNote: 'bounded census',
    },
  ])
  assert.equal(s.channels.length, 2)
  const im = s.channels.find((c) => c.source === 'apple_messages')
  assert.equal(im?.channel, 'imessage')
  assert.equal(im?.observedCommunicationCount, 5)
  assert.equal(im?.inboundCount, 3)
  assert.equal(im?.outboundCount, 2)
  assert.equal(im?.lastObservedAt, '2026-08-25T20:42:00.000Z')
  assert.equal(im?.twoWay, true)
  const email = s.channels.find((c) => c.source === 'gmail_contacts')
  assert.equal(email?.channel, 'email')
  assert.equal(email?.coverageLimited, true)
  // newest-first
  assert.equal(s.channels[0].source, 'apple_messages')
})

test('channels: empty evidence -> no channels (aggregate still usable)', () => {
  const s = summarizeRelationshipEvidence([])
  assert.deepEqual(s.channels, [])
  assert.equal(s.observedCommunicationCount, 0)
})
