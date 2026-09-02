import { test } from 'node:test'
import assert from 'node:assert/strict'
import {
  GMAIL_CONTEXT_SOURCE,
  gmailMetadataToContext,
  headerEmails,
} from '../../lib/relationship-intel/gmail-latest-context'

function message(headers: Record<string, string>, overrides: Record<string, unknown> = {}) {
  return {
    id: 'gmail-message-1',
    threadId: 'gmail-thread-1',
    internalDate: '1772384702000',
    payload: { headers: Object.entries(headers).map(([name, value]) => ({ name, value })) },
    ...overrides,
  }
}

test('parses named and bare RFC-style mailbox headers', () => {
  assert.deepEqual(headerEmails('Ami Beach <shadlefarm@gmail.com>, lisa@example.com'), [
    'shadlefarm@gmail.com',
    'lisa@example.com',
  ])
})

test('maps an inbound Gmail header to a subject-only canonical interaction', () => {
  const result = gmailMetadataToContext(
    message({
      From: 'Alicia Geigel <alicia.geigel@gmail.com>',
      To: 'Lisa Penfield <penfield33@gmail.com>',
      Subject: '  Datiles clips + flamenco pics  ',
    }),
    'alicia.geigel@gmail.com',
    'penfield33@gmail.com',
    'person-alicia',
  )
  assert.equal(result.ok, true)
  if (!result.ok) return
  assert.equal(result.interaction.personId, 'person-alicia')
  assert.equal(result.interaction.title, 'Datiles clips + flamenco pics')
  assert.equal(result.interaction.summary, undefined)
  assert.equal(result.interaction.direction, 'inbound')
  assert.equal(result.interaction.sourceSystem, GMAIL_CONTEXT_SOURCE)
  assert.equal(result.interaction.sourceExternalId, 'gmail-message-1')
  assert.equal(JSON.stringify(result.interaction).includes('body'), false)
})

test('maps a one-to-one outbound header and rejects group attribution', () => {
  const direct = gmailMetadataToContext(
    message({ From: 'Lisa <penfield33@gmail.com>', To: 'Ami <shadlefarm@gmail.com>', Subject: 'Hello Ami' }),
    'shadlefarm@gmail.com',
    'penfield33@gmail.com',
    'person-ami',
  )
  assert.equal(direct.ok && direct.interaction.direction, 'outbound')

  const group = gmailMetadataToContext(
    message({ From: 'Lisa <penfield33@gmail.com>', To: 'Ami <shadlefarm@gmail.com>, Other <other@example.com>', Subject: 'Group note' }),
    'shadlefarm@gmail.com',
    'penfield33@gmail.com',
    'person-ami',
  )
  assert.deepEqual(group, { ok: false, reason: 'ambiguous_outbound' })
})

test('skips missing subjects and unrelated envelopes', () => {
  const noSubject = gmailMetadataToContext(
    message({ From: 'Ami <shadlefarm@gmail.com>', To: 'Lisa <penfield33@gmail.com>' }),
    'shadlefarm@gmail.com',
    'penfield33@gmail.com',
    'person-ami',
  )
  assert.deepEqual(noSubject, { ok: false, reason: 'missing_subject' })

  const unrelated = gmailMetadataToContext(
    message({ From: 'Other <other@example.com>', To: 'Lisa <penfield33@gmail.com>', Subject: 'No match' }),
    'shadlefarm@gmail.com',
    'penfield33@gmail.com',
    'person-ami',
  )
  assert.deepEqual(unrelated, { ok: false, reason: 'not_target_correspondence' })
})
