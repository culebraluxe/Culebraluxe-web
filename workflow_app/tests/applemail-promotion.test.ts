import assert from 'node:assert/strict'
import test from 'node:test'

import { normalizeLandedMail, parseSenderAddress, type LandedAppleMailRow } from '../../lib/relationship-intel/applemail'

const INTERNAL = new Set(['lisa@culebraluxe.com', 'penfield33@gmail.com'])

function row(overrides: Partial<LandedAppleMailRow> = {}): LandedAppleMailRow {
  return {
    source_account: 'lisa@culebraluxe.com',
    source_message_id: 'message-id:one@example.com',
    mailbox_kind: 'inbox',
    mailbox_name: 'INBOX',
    local_id: 101,
    message_id: 'one@example.com',
    occurred_at: '2026-09-01T12:00:00.000Z',
    sender: 'Dana <dana@example.com>',
    to_recipients: [{ address: 'lisa@culebraluxe.com', name: 'Lisa' }],
    cc_recipients: [],
    bcc_recipients: [],
    subject: 'Hello',
    ...overrides,
  }
}

test('applemail promotion: an inbox row becomes an inbound observation of the sender', () => {
  const { observations, skipped } = normalizeLandedMail([row()], INTERNAL)
  assert.equal(observations.length, 1)
  assert.equal(observations[0].direction, 'inbound')
  assert.equal(observations[0].externalEmail, 'dana@example.com')
  assert.equal(observations[0].displayName, 'Dana')
  // The landed identity is carried through, so interaction replay matches landing.
  assert.equal(observations[0].sourceExternalId, 'message-id:one@example.com')
  assert.equal(observations[0].sourceAccount, 'lisa@culebraluxe.com')
  assert.equal(observations[0].uid, 101)
  assert.equal(skipped.internal_only, 0)
})

test('applemail promotion: a mail from an internal address is not a relationship', () => {
  const { observations, skipped } = normalizeLandedMail(
    [row({ sender: 'Penfield <penfield33@gmail.com>' })],
    INTERNAL,
  )
  assert.equal(observations.length, 0)
  assert.equal(skipped.internal_only, 1)
})

test('applemail promotion: a sent row becomes outbound to the single external recipient', () => {
  const { observations } = normalizeLandedMail(
    [
      row({
        mailbox_kind: 'sent',
        source_message_id: 'message-id:two@example.com',
        sender: 'Lisa <lisa@culebraluxe.com>',
        to_recipients: [
          { address: 'lisa@culebraluxe.com', name: 'Lisa' },
          { address: 'buyer@example.com', name: 'Buyer' },
        ],
      }),
    ],
    INTERNAL,
  )
  assert.equal(observations.length, 1)
  assert.equal(observations[0].direction, 'outbound')
  assert.equal(observations[0].externalEmail, 'buyer@example.com')
})

test('applemail promotion: two external recipients is ambiguous, never guessed', () => {
  const { observations, skipped } = normalizeLandedMail(
    [
      row({
        mailbox_kind: 'sent',
        to_recipients: [
          { address: 'a@example.com', name: null },
          { address: 'b@example.com', name: null },
        ],
      }),
    ],
    INTERNAL,
  )
  assert.equal(observations.length, 0)
  assert.equal(skipped.ambiguous, 1)
})

test('applemail promotion: a sent row with no external recipient is internal only', () => {
  const { observations, skipped } = normalizeLandedMail(
    [row({ mailbox_kind: 'sent', to_recipients: [{ address: 'lisa@culebraluxe.com', name: null }] })],
    INTERNAL,
  )
  assert.equal(observations.length, 0)
  assert.equal(skipped.internal_only, 1)
})

test('applemail promotion: a row with no timestamp is skipped, not defaulted to now', () => {
  const { observations, skipped } = normalizeLandedMail([row({ occurred_at: null })], INTERNAL)
  assert.equal(observations.length, 0)
  assert.equal(skipped.no_timestamp, 1)
})

test('applemail promotion: the same message in two mailboxes collapses to one observation', () => {
  const { observations, skipped } = normalizeLandedMail(
    [row(), row({ mailbox_name: 'Archive', local_id: 202 })],
    INTERNAL,
  )
  assert.equal(observations.length, 1)
  assert.equal(skipped.duplicate, 1)
})

test('applemail promotion: a row with no usable sender address is unaddressed', () => {
  const { observations, skipped } = normalizeLandedMail([row({ sender: 'no-reply' })], INTERNAL)
  assert.equal(observations.length, 0)
  assert.equal(skipped.unaddressed, 1)
})

test('applemail promotion: driver-native occurred_at types normalize to ISO', () => {
  const date = new Date('2026-08-15T09:30:00.000Z')
  const { observations } = normalizeLandedMail([row({ occurred_at: date })], INTERNAL)
  assert.equal(observations[0].occurredAt, '2026-08-15T09:30:00.000Z')
})

test('applemail promotion: the subject is bounded and control characters stripped', () => {
  const { observations } = normalizeLandedMail([row({ subject: '  Re:\u0000   spaced  \u0007  ' })], INTERNAL)
  assert.equal(observations[0].subject, 'Re: spaced')
})

test('applemail promotion: sender parsing handles bracketed, bare and unparseable values', () => {
  assert.deepEqual(parseSenderAddress('Dana <dana@example.com>'), { address: 'dana@example.com', name: 'Dana' })
  assert.deepEqual(parseSenderAddress('"Dana Q" <DANA@Example.com>'), { address: 'dana@example.com', name: 'Dana Q' })
  assert.deepEqual(parseSenderAddress('dana@example.com'), { address: 'dana@example.com', name: null })
  assert.equal(parseSenderAddress('not an address'), null)
  assert.equal(parseSenderAddress(null), null)
})
