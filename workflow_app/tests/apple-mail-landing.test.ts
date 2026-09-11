import assert from 'node:assert/strict'
import test from 'node:test'

import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import { appleMailReplayId, landAppleMail } from '../../db/landing'

/** Capture what the landing writer actually sends, without a database. */
function capture() {
  const calls: Array<{ sql: string; values: unknown[] }> = []
  const executor = (async (strings: TemplateStringsArray, ...values: unknown[]) => {
    calls.push({ sql: strings.join(' ').replace(/\s+/g, ' '), values })
    return [{ id: 'row-1' }] as QueryRow[]
  }) as QueryExecutor
  return { calls, executor }
}

function captureNoInsert() {
  const executor = (async () => [] as QueryRow[]) as QueryExecutor
  return executor
}

const RECORD = {
  mailbox: 'inbox' as const,
  mailboxName: 'INBOX',
  localId: 4242,
  messageId: '<abc@culebraluxe.com>',
  occurredAt: '2026-09-11T10:00:00.000Z',
  sender: 'Someone <someone@example.com>',
  to: [{ address: 'lisa@culebraluxe.com', name: 'Lisa' }],
  cc: [{ address: 'cc@example.com', name: null }],
  bcc: [{ address: 'bcc@example.com', name: null }],
  subject: 'Hello',
}

const ACCOUNT = 'lisa@culebraluxe.com'

// --- replay key (work order items 3 and 4) -----------------------------------

test('apple mail replay key prefers the RFC Message-ID', () => {
  assert.equal(
    appleMailReplayId({ messageId: '<abc@x>', mailboxKind: 'inbox', localId: 7 }),
    'message-id:<abc@x>',
  )
})

test('apple mail replay key falls back to the stable local identity', () => {
  assert.equal(
    appleMailReplayId({ messageId: null, mailboxKind: 'sent', localId: 99 }),
    'mail-local:sent:99',
  )
  assert.equal(appleMailReplayId({ messageId: '   ', mailboxKind: 'inbox', localId: 1 }), 'mail-local:inbox:1')
})

test('apple mail replay key is never invented', () => {
  assert.equal(appleMailReplayId({ messageId: null, mailboxKind: null, localId: 5 }), null)
  assert.equal(appleMailReplayId({ messageId: null, mailboxKind: 'inbox', localId: null }), null)
  assert.equal(appleMailReplayId({}), null)
})

// --- landing (work order items 1, 5, 6, 7, 8, 11) ----------------------------

test('an Apple Mail record lands in l_applemail with its configured account', async () => {
  const { calls, executor } = capture()
  const inserted = await landAppleMail(
    {
      sourceAccount: ACCOUNT,
      sourceMessageId: appleMailReplayId({ messageId: RECORD.messageId, ...{} })!,
      mailboxKind: RECORD.mailbox,
      mailboxName: RECORD.mailboxName,
      localId: RECORD.localId,
      messageId: RECORD.messageId,
      occurredAt: RECORD.occurredAt,
      sender: RECORD.sender,
      toRecipients: RECORD.to,
      ccRecipients: RECORD.cc,
      bccRecipients: RECORD.bcc,
      subject: RECORD.subject,
      raw: RECORD,
    },
    executor,
  )
  assert.equal(inserted, true)
  const sql = calls[0].sql
  assert.match(sql, /insert into l_applemail/)
  assert.match(sql, /on conflict \(coalesce\(source_account, ''\), source_message_id\) do nothing/)
  assert.match(sql, /returning id/)
  assert.ok(calls[0].values.includes(ACCOUNT), 'source_account must be the configured mailbox')
  assert.ok(calls[0].values.includes('message-id:<abc@culebraluxe.com>'))
  assert.ok(calls[0].values.includes('INBOX'))
  assert.ok(calls[0].values.includes(4242))
  assert.ok(calls[0].values.includes('Hello'))
})

test('recipients are preserved as json, including cc and bcc', async () => {
  const { calls, executor } = capture()
  await landAppleMail(
    {
      sourceAccount: ACCOUNT,
      sourceMessageId: 'message-id:<abc@x>',
      toRecipients: RECORD.to,
      ccRecipients: RECORD.cc,
      bccRecipients: RECORD.bcc,
      raw: RECORD,
    },
    executor,
  )
  const values = calls[0].values.map((v) => (typeof v === 'string' ? v : ''))
  const jsonBlobs = values.filter((v) => v.startsWith('['))
  assert.equal(jsonBlobs.length, 3, 'to, cc and bcc each survive as their own json value')
  assert.match(jsonBlobs.join(' '), /cc@example\.com/)
  assert.match(jsonBlobs.join(' '), /bcc@example\.com/)
})

test('raw carries the untouched exporter record, not a derived object', async () => {
  const { calls, executor } = capture()
  await landAppleMail(
    { sourceAccount: ACCOUNT, sourceMessageId: 'message-id:<abc@x>', raw: RECORD },
    executor,
  )
  const rawJson = calls[0].values.find(
    (v) => typeof v === 'string' && v.includes('mailboxName') && v.includes('localId'),
  ) as string
  assert.ok(rawJson, 'the raw value must be the exporter record as it arrived')
  assert.deepEqual(JSON.parse(rawJson), RECORD)
})

test('a replayed record reports false rather than claiming an insert', async () => {
  const inserted = await landAppleMail(
    { sourceAccount: ACCOUNT, sourceMessageId: 'message-id:<abc@x>', raw: RECORD },
    captureNoInsert(),
  )
  assert.equal(inserted, false)
})

// --- privacy boundary (work order item 11) -----------------------------------

test('the landing write acquires no body, snippet, attachment or MIME field', async () => {
  const { calls, executor } = capture()
  await landAppleMail(
    { sourceAccount: ACCOUNT, sourceMessageId: 'message-id:<abc@x>', raw: RECORD },
    executor,
  )
  const sql = calls[0].sql.toLowerCase()
  for (const forbidden of ['body', 'snippet', 'attachment', 'mime', 'preview']) {
    assert.equal(sql.includes(forbidden), false, `l_applemail must not carry ${forbidden}`)
  }
})

// --- separation from Gmail (work order item 12) ------------------------------

test('Apple Mail never writes into the Gmail landing table', async () => {
  const { calls, executor } = capture()
  await landAppleMail(
    { sourceAccount: ACCOUNT, sourceMessageId: 'message-id:<abc@x>', raw: RECORD },
    executor,
  )
  assert.equal(calls[0].sql.includes('l_email'), false)
  assert.equal(calls[0].sql.includes('l_apple_mail'), false)
})
