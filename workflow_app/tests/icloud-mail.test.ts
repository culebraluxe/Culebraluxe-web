import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  boundedEmailSubject,
  buildICloudMailEvidence,
  classifyEnvelope,
  ICLOUD_MAIL_SOURCE,
  observationToInteraction,
  type ICloudMailObservation,
} from '../../lib/relationship-intel/icloud-mail'

const internal = new Set(['penfield33@gmail.com', 'lisa@culebraluxe.com'])

test('classifies inbound and direct outbound using all internal mailbox identities', () => {
  assert.deepEqual(
    classifyEnvelope({
      from: [{ name: 'Alicia', address: 'alicia@example.com' }],
      to: [{ address: 'lisa@culebraluxe.com' }],
    }, internal),
    { ok: true, direction: 'inbound', externalEmail: 'alicia@example.com', displayName: 'Alicia' },
  )
  assert.deepEqual(
    classifyEnvelope({
      from: [{ address: 'lisa@culebraluxe.com' }],
      to: [{ name: 'Ami', address: 'ami@example.com' }],
    }, internal),
    { ok: true, direction: 'outbound', externalEmail: 'ami@example.com', displayName: 'Ami' },
  )
})

test('rejects group outbound mail instead of attributing it to one Person', () => {
  assert.deepEqual(
    classifyEnvelope({
      from: [{ address: 'lisa@culebraluxe.com' }],
      to: [{ address: 'a@example.com' }, { address: 'b@example.com' }],
    }, internal),
    { ok: false, reason: 'ambiguous' },
  )
})

test('subject is bounded and interaction stores metadata only', () => {
  assert.equal(boundedEmailSubject('  Re:  Sea\n to Soul '), 'Re: Sea to Soul')
  const observation: ICloudMailObservation = {
    sourceExternalId: 'message-id:<one@example.com>',
    sourceAccount: 'lisa@culebraluxe.com',
    mailbox: 'INBOX',
    uid: 4,
    uidValidity: '10',
    occurredAt: '2026-09-02T12:00:00.000Z',
    direction: 'inbound',
    externalEmail: 'alicia@example.com',
    displayName: 'Alicia',
    subject: 'Re: Sea to Soul',
  }
  const interaction = observationToInteraction(observation, 'person-1')
  assert.equal(interaction.sourceSystem, ICLOUD_MAIL_SOURCE)
  assert.equal(interaction.title, 'Re: Sea to Soul')
  assert.equal(interaction.summary, undefined)
  assert.equal(JSON.stringify(interaction).includes('body'), false)
})

test('evidence aggregates exact direct metadata per external identity', () => {
  const base = {
    sourceAccount: 'lisa@culebraluxe.com',
    mailbox: 'INBOX',
    uidValidity: '10',
    externalEmail: 'alicia@example.com',
    displayName: 'Alicia',
    subject: 'Subject',
  }
  const result = buildICloudMailEvidence([
    { ...base, sourceExternalId: '1', uid: 1, occurredAt: '2026-09-01T12:00:00.000Z', direction: 'inbound' },
    { ...base, sourceExternalId: '2', uid: 2, occurredAt: '2026-09-02T12:00:00.000Z', direction: 'outbound' },
  ])
  assert.equal(result.length, 1)
  assert.equal(result[0].evidence.source, ICLOUD_MAIL_SOURCE)
  assert.equal(result[0].evidence.inboundCount, 1)
  assert.equal(result[0].evidence.outboundCount, 1)
  assert.equal(result[0].evidence.isTwoWay, true)
  assert.match(result[0].evidence.coverageNote ?? '', /bodies, snippets, attachments/)
})
