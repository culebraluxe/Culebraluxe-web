import assert from 'node:assert/strict'
import test from 'node:test'

import { materializeAppleMessages } from '../../db/apple-message-materialization'
import { createInteractionsBatch } from '../../db/interactions'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import type { AppleMessagesExport } from '../../lib/relationship-intel/apple-messages'

// ---------------------------------------------------------------------------
// The 5-hour bug: this loop wrote one row at a time, so a 93,000-message store
// paid ~186,000 sequential round trips (measured at 68.7ms each) for work the
// database finishes in ~70ms. These tests pin the fix: the number of STATEMENTS
// must scale with chunks, not with messages.
// ---------------------------------------------------------------------------

const MESSAGES = 1_200
const CHUNK = 500

function buildExport(): AppleMessagesExport {
  const handles = [{ id: '+17875550134', rowid: 1 }]
  const messages = Array.from({ length: MESSAGES }, (_, index) => ({
    rowid: index + 1,
    guid: `guid-${index + 1}`,
    handleId: 1,
    handleValue: '+17875550134',
    chatGuid: 'iMessage;-;+17875550134',
    isFromMe: index % 2 === 0,
    dateISO: '2026-08-01T12:00:00.000Z',
    service: 'iMessage',
    text: `message ${index + 1}`,
  }))
  return { sourceAccount: 'lisapenfield@icloud.com', handles, messages } as unknown as AppleMessagesExport
}

/** Counts statements and answers each shape the materializer needs. */
function fakeExecutor(options: { alreadyLanded?: boolean } = {}) {
  const statements: string[] = []
  const counts = { landedRows: 0, interactionRows: 0 }

  const execute = (async (strings: TemplateStringsArray, ...params: unknown[]) => {
    const sqlText = strings.join(' $ ').replace(/\s+/g, ' ')
    if (sqlText.includes('integration_relationship_evidence')) {
      statements.push('evidence')
      return [
        {
          id: 'ev-1',
          source: 'apple_messages',
          source_account: 'lisapenfield@icloud.com',
          source_identity_key: '+17875550134',
          source_label: 'iMessage',
          review_state: 'exact_linked',
          canonical_person_id: 'person-1',
        },
      ] as unknown as QueryRow[]
    }
    if (sqlText.includes('insert into l_imessage')) {
      statements.push('land')
      const first = params[0] as unknown as string[]
      counts.landedRows += first.length
      if (options.alreadyLanded) return [] as QueryRow[]
      return first.map(() => ({ id: 'row' })) as unknown as QueryRow[]
    }
    if (sqlText.includes('insert into interaction')) {
      statements.push('interaction')
      const first = params[0] as unknown as string[]
      counts.interactionRows += first.length
      if (options.alreadyLanded) return [] as QueryRow[]
      return first.map(() => ({ id: 'row' })) as unknown as QueryRow[]
    }
    if (sqlText.includes('refresh materialized view')) {
      statements.push('refresh')
      return [] as QueryRow[]
    }
    statements.push(`unexpected: ${sqlText.slice(0, 60)}`)
    return [] as QueryRow[]
  }) as QueryExecutor

  return { execute, statements, counts }
}

test('materialization writes in chunks, not one round trip per message', async () => {
  const { execute, statements, counts } = fakeExecutor()
  const result = await materializeAppleMessages(buildExport(), execute, { refresh: async () => {} })

  const land = statements.filter((s) => s === 'land').length
  const interaction = statements.filter((s) => s === 'interaction').length
  const expectedChunks = Math.ceil(MESSAGES / CHUNK)

  assert.equal(result.eventsSeen, MESSAGES)
  assert.equal(result.inserted, MESSAGES)
  assert.equal(result.errors, 0)
  assert.equal(counts.landedRows, MESSAGES, 'every message still lands')
  assert.equal(counts.interactionRows, MESSAGES, 'every message still materializes')
  assert.equal(land, expectedChunks, `landing statements should be ${expectedChunks}, not ${MESSAGES}`)
  assert.equal(interaction, expectedChunks, `interaction statements should be ${expectedChunks}`)
  // The whole point: statements scale with chunks (a few), never with messages.
  assert.ok(
    statements.length < 20,
    `expected a handful of statements for ${MESSAGES} messages, got ${statements.length}`,
  )
})

test('re-running is replay-safe and still costs a handful of statements', async () => {
  const { execute, statements } = fakeExecutor({ alreadyLanded: true })
  const result = await materializeAppleMessages(buildExport(), execute)

  assert.equal(result.inserted, 0, 'nothing new on a second run')
  assert.equal(result.replayed, MESSAGES, 'every message is recognised as already present')
  assert.ok(statements.length < 20, `replay should not fan out per row, got ${statements.length}`)
})

test('createInteractionsBatch counts what it cannot accept and still writes the rest', async () => {
  const statements: string[] = []
  const execute = (async (strings: TemplateStringsArray, ...params: unknown[]) => {
    statements.push(strings.join(' ').replace(/\s+/g, ' ').trim())
    const ids = params[0] as unknown as string[]
    return ids.map(() => ({ id: 'row' })) as unknown as QueryRow[]
  }) as QueryExecutor

  const base = {
    personId: 'p1',
    channel: 'imessage',
    eventType: 'message',
    occurredAt: '2026-08-01T12:00:00.000Z',
    sourceSystem: 'apple_messages',
    sourceExternalId: 'g1',
    sourceMetadata: {},
  }

  const result = await createInteractionsBatch(
    [
      base,
      { ...base, sourceExternalId: 'g2' },
      // A HALF identity is the combination the single-row form refuses; both absent
      // is legitimate (a manual interaction), so the batch must match that exactly.
      { ...base, sourceExternalId: undefined },
      { ...base, personId: '' },
    ],
    execute,
  )

  assert.equal(result.inserted, 2)
  assert.equal(result.rejected, 2)
  assert.equal(statements.length, 1, 'one statement for the whole accepted batch')
  // The batch is one set-based statement, not one per row.
  assert.ok(statements[0].includes('unnest('), 'the batch uses unnest')
  assert.ok(statements[0].includes('do nothing'), 'and stays replay-safe')
})

test('nothing is materialized without an authoritative Person link', async () => {
  const execute = (async (strings: TemplateStringsArray) => {
    const sqlText = strings.join(' ')
    if (sqlText.includes('integration_relationship_evidence')) return [] as QueryRow[]
    throw new Error(`unexpected statement: ${sqlText.slice(0, 60)}`)
  }) as QueryExecutor

  const result = await materializeAppleMessages(buildExport(), execute)
  assert.equal(result.inserted, 0)
  assert.equal(result.unmatchedOrAmbiguousHandles, 1)
})
