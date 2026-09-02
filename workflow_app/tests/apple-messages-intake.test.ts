import { test } from 'node:test'
import assert from 'node:assert/strict'

import {
  buildMessagesRelationshipEvidence,
  handleToIdentities,
  type AppleMessagesExport,
} from '../../lib/relationship-intel/apple-messages'

// ---------------------------------------------------------------------------
// Apple Messages -> neutral ODS adapter (pure). No database.
// ---------------------------------------------------------------------------

function sample(): AppleMessagesExport {
  return {
    sourceAccount: 'local_mac_proof',
    handles: [
      { rowid: 1, id: '+17875550134', country: 'us', service: 'iMessage', uncanonicalizedId: '+1 (787) 555-0134', personCentricId: null },
    ],
    messages: [
      { rowid: 1, guid: 'm1', chatGuid: 'c1', handleId: 1, handleValue: '+17875550134', service: 'iMessage', date: null, dateISO: '2023-01-02T10:00:00.000Z', isFromMe: 0, text: 'hi', hasAttachments: 0 },
      { rowid: 2, guid: 'm2', chatGuid: 'c1', handleId: 1, handleValue: '+17875550134', service: 'iMessage', date: null, dateISO: '2023-01-02T10:05:00.000Z', isFromMe: 1, text: 'hey', hasAttachments: 0 },
      { rowid: 3, guid: 'm3', chatGuid: 'c1', handleId: 1, handleValue: '+17875550134', service: 'iMessage', date: null, dateISO: '2023-01-03T09:00:00.000Z', isFromMe: 0, text: 'showing?', hasAttachments: 1 },
    ],
  }
}

test('apple-messages: phone handle maps to a normalized phone identity', () => {
  const { phones, emails } = handleToIdentities('+1 (787) 555-0134')
  assert.equal(phones.length, 1)
  assert.equal(phones[0].normalized, '+17875550134')
  assert.equal(emails.length, 0)
})

test('apple-messages: email handle maps to a normalized email identity', () => {
  const { emails, phones } = handleToIdentities('Jane@Example.com')
  assert.equal(emails.length, 1)
  assert.equal(emails[0].normalized, 'jane@example.com')
  assert.equal(phones.length, 0)
})

test('apple-messages: one handle -> one neutral evidence row with correct aggregates', () => {
  const rows = buildMessagesRelationshipEvidence(sample())
  assert.equal(rows.length, 1)
  const { evidence } = rows[0]
  assert.equal(evidence.source, 'apple_messages')
  assert.equal(evidence.sourceIdentityKey, '+17875550134')
  assert.equal(evidence.inboundCount, 2)
  assert.equal(evidence.outboundCount, 1)
  assert.equal(evidence.isTwoWay, true)
  assert.equal(evidence.firstObservedAt, '2023-01-02T10:00:00.000Z')
  assert.equal(evidence.lastObservedAt, '2023-01-03T09:00:00.000Z')
  assert.equal(evidence.hasPhone, true)
})

test('apple-messages: replay fingerprint is deterministic (idempotent source identity)', () => {
  const a = buildMessagesRelationshipEvidence(sample())
  const b = buildMessagesRelationshipEvidence(sample())
  assert.equal(a[0].fingerprint, b[0].fingerprint)
})

test('apple-messages: changed message set changes the fingerprint (replay distinguishable)', () => {
  const a = buildMessagesRelationshipEvidence(sample())
  const changed = sample()
  changed.messages[0].dateISO = '2023-06-01T00:00:00.000Z'
  const b = buildMessagesRelationshipEvidence(changed)
  assert.notEqual(a[0].fingerprint, b[0].fingerprint)
})

test('apple-messages: duplicate numeric handles for one identity aggregate before evidence upsert', () => {
  const duplicated = sample()
  duplicated.handles.push({
    rowid: 2,
    id: '+17875550134',
    country: 'us',
    service: 'SMS',
    uncanonicalizedId: '7875550134',
    personCentricId: null,
  })
  duplicated.messages.push(
    { rowid: 4, guid: 'm4', chatGuid: 'c2', handleId: 2, handleValue: '+17875550134', service: 'SMS', date: null, dateISO: '2023-01-04T09:00:00.000Z', isFromMe: 0, text: 'later', hasAttachments: 0 },
    { rowid: 5, guid: 'm5', chatGuid: 'c2', handleId: 2, handleValue: '+17875550134', service: 'SMS', date: null, dateISO: '2023-01-04T09:05:00.000Z', isFromMe: 1, text: 'reply', hasAttachments: 0 },
    { rowid: 6, guid: 'm6', chatGuid: 'chat123;+;group', handleId: 2, handleValue: '+17875550134', service: 'SMS', date: null, dateISO: '2023-01-04T10:00:00.000Z', isFromMe: 0, text: 'group', hasAttachments: 0 },
  )

  const rows = buildMessagesRelationshipEvidence(duplicated)
  assert.equal(rows.length, 1, 'one textual identity produces one evidence upsert')
  assert.equal(rows[0].evidence.inboundCount, 3)
  assert.equal(rows[0].evidence.outboundCount, 2)
  assert.equal(rows[0].evidence.lastObservedAt, '2023-01-04T09:05:00.000Z')
})

// --- Real-data-driven fixes ---

test('apple-messages: unclassifiable handle produces NO fabricated identity (real urn/name handles)', () => {
  const { phones, emails } = handleToIdentities('urn:biz:e46750f1-3f94-4aba-73ca-1c14fb3adddd')
  assert.equal(phones.length, 0)
  assert.equal(emails.length, 0)
  const short = handleToIdentities('bj')
  assert.equal(short.phones.length, 0)
  assert.equal(short.emails.length, 0)
})

test('apple-messages: counts are derived from ALL messages even when timestamps are null', () => {
  // Mirrors the real export where dateISO was null (Swift INTEGER bug, now fixed);
  // direction must still yield correct inbound/outbound/two-way.
  const ex: AppleMessagesExport = {
    sourceAccount: 'local',
    handles: [{ rowid: 1, id: '+17875550134', country: 'us', service: 'iMessage', uncanonicalizedId: null, personCentricId: null }],
    messages: [
      { rowid: 1, guid: 'm1', chatGuid: 'c1', handleId: 1, handleValue: '+17875550134', service: 'iMessage', date: null, dateISO: null, isFromMe: 0, text: 'a', hasAttachments: 0 },
      { rowid: 2, guid: 'm2', chatGuid: 'c1', handleId: 1, handleValue: '+17875550134', service: 'iMessage', date: null, dateISO: null, isFromMe: 1, text: 'b', hasAttachments: 0 },
    ],
  }
  const { evidence } = buildMessagesRelationshipEvidence(ex)[0]
  assert.equal(evidence.inboundCount, 1)
  assert.equal(evidence.outboundCount, 1)
  assert.equal(evidence.isTwoWay, true)
  assert.equal(evidence.firstObservedAt, null)
  assert.equal(evidence.lastObservedAt, null)
})
