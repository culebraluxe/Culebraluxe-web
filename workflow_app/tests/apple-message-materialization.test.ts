import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  appleServiceToChannel,
  boundedPreview,
  mapAppleMessageToInteraction,
  resolveHandlePerson,
  APPLE_PREVIEW_MAX_LENGTH,
} from '../../lib/relationship-intel/apple-message-materializer'
import {
  APPLE_MESSAGES_SOURCE,
  isGroupChatGuid,
  appleNanosToIso,
} from '../../lib/relationship-intel/apple-messages'
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

test('mapAppleMessageToInteraction: inbound -> channel/direction/replay key, bounded preview only', () => {
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
  assert.equal(input.sourceExternalId, 'GUID-IN-1')
  assert.equal(input.title, undefined)
  assert.equal(input.summary, 'SECRET BODY')
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

test('mapAppleMessageToInteraction: no usable text -> neutral labels, never fabricated prose', () => {
  const noText = mapAppleMessageToInteraction(
    msg({ guid: 'GUID-NO-TEXT', isFromMe: 0, text: null, hasAttachments: 0 }),
    'person-1',
    'local_mac',
  )
  assert.equal(noText.summary, 'Message')

  const attachmentOnly = mapAppleMessageToInteraction(
    msg({ guid: 'GUID-ATT', isFromMe: 1, text: '', hasAttachments: 1 }),
    'person-1',
    'local_mac',
  )
  assert.equal(attachmentOnly.summary, 'Attachment')
})

test('boundedPreview: collapses whitespace/newlines, trims, and caps length', () => {
  assert.equal(boundedPreview('  Let\'s  meet\nbefore\tyou leave  '), 'Let\'s meet before you leave')
  const long = 'a'.repeat(500)
  const preview = boundedPreview(long)
  assert.ok(preview !== null)
  assert.ok(preview!.length <= APPLE_PREVIEW_MAX_LENGTH)
  assert.equal(preview!.endsWith('…'), true)
  assert.equal(preview!.includes('\n'), false)
  assert.equal(boundedPreview(null), null)
  assert.equal(boundedPreview(undefined), null)
  assert.equal(boundedPreview(''), null)
  assert.equal(boundedPreview('   '), null)
})

test('appleNanosToIso: Apple INTEGER-nanoseconds timestamp conversion', () => {
  assert.equal(appleNanosToIso(0), '2001-01-01T00:00:00.000Z')
  assert.equal(appleNanosToIso(1_000_000_000), '2001-01-01T00:00:01.000Z')
  const mid2026 = appleNanosToIso(809312575831244416)
  assert.equal(mid2026 !== null, true)
  assert.equal(new Date(mid2026 as string).getUTCFullYear(), 2026)
  assert.equal(appleNanosToIso(null), null)
})

test('isGroupChatGuid: group chats excluded from person-specific materialization', () => {
  assert.equal(isGroupChatGuid('any;-;+17875550134'), false)
  assert.equal(isGroupChatGuid(null), false)
  assert.equal(isGroupChatGuid('any;-;'), false)
  assert.equal(isGroupChatGuid('any;-;+18609895020'), false)
  assert.equal(isGroupChatGuid('any;-;group;ABC123'), true)
  assert.equal(isGroupChatGuid('any;-;GROUP;ABC123'), true)
  assert.equal(isGroupChatGuid('any;+;chat671086128536283356'), true)
  assert.equal(isGroupChatGuid('any;+;d9cccba7dc674c3e9c149038c45404a6'), true)
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
  assert.equal(resolveHandlePerson(map, '+17875550987').ok, false)
  assert.equal(resolveHandlePerson(map, '+17875550000').ok, false)
  assert.equal(resolveHandlePerson(map, '+17875551111').ok, false)
  assert.equal(resolveHandlePerson(map, '+17875552222').ok, false)
  assert.equal(resolveHandlePerson(map, '+999').ok, false)
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
  const email = s.channels.find((c) => c.source === 'email')
  assert.equal(email?.channel, 'email')
  assert.equal(email?.coverageLimited, true)
  assert.equal(s.channels[0].source, 'apple_messages')
})

test('channels: empty evidence -> no channels (aggregate still usable)', () => {
  const s = summarizeRelationshipEvidence([])
  assert.deepEqual(s.channels, [])
  assert.equal(s.observedCommunicationCount, 0)
})
