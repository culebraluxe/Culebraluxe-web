import assert from 'node:assert/strict'
import test from 'node:test'

import { materializeAppleMessages } from '../../db/apple-message-materialization'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import type { AppleMessagesExport } from '../../lib/relationship-intel/apple-messages'

// ---------------------------------------------------------------------------
// ODS takes everything; the warehouse takes what the screen contract needs.
//
// Two separate bugs are pinned here:
//   1. LOGISTICS - landing used to be one round trip per message (68.7ms each,
//      measured), so a 93,000-message store was hours of pure latency.
//   2. GRAIN - every message used to become a warehouse interaction, so Ami's
//      5,519 messages became 5,519 rows to feed a pane that shows 4.
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
    // Ascending dates, so the newest message is the last one processed.
    dateISO: new Date(Date.UTC(2026, 7, 1, 0, 0, index)).toISOString(),
    service: 'iMessage',
    text: `message ${index + 1}`,
  }))
  return { sourceAccount: 'lisapenfield@icloud.com', handles, messages } as unknown as AppleMessagesExport
}

/** Records each statement with its kind and text, and answers the shapes needed. */
function fakeExecutor(options: { alreadyLanded?: boolean; upsertReturnsRow?: boolean } = {}) {
  const kinds: string[] = []
  const sql: Record<string, string> = {}
  const counts = { landedRows: 0, latestUpserts: 0 }

  const execute = (async (strings: TemplateStringsArray, ...params: unknown[]) => {
    const sqlText = strings.join(' $ ').replace(/\s+/g, ' ')
    if (sqlText.includes('integration_relationship_evidence')) {
      kinds.push('evidence')
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
      kinds.push('land')
      const ids = params[0] as unknown as string[]
      counts.landedRows += ids.length
      if (options.alreadyLanded) return [] as QueryRow[]
      return ids.map(() => ({ id: 'row' })) as unknown as QueryRow[]
    }
    if (sqlText.includes('insert into interaction')) {
      kinds.push('latest')
      sql.latest = sqlText
      counts.latestUpserts += 1
      if (options.upsertReturnsRow === false) return [] as QueryRow[]
      return [{ inserted: true }] as unknown as QueryRow[]
    }
    if (sqlText.includes('refresh materialized view')) {
      kinds.push('refresh')
      return [] as QueryRow[]
    }
    kinds.push(`unexpected: ${sqlText.slice(0, 60)}`)
    return [] as QueryRow[]
  }) as QueryExecutor

  return { execute, kinds, sql, counts }
}

test('every message lands in ODS, but only the newest per source reaches the warehouse', async () => {
  const { execute, kinds, sql } = fakeExecutor()
  const result = await materializeAppleMessages(buildExport(), execute, { refresh: async () => {} })

  assert.equal(result.eventsSeen, MESSAGES)
  assert.equal(result.landed, MESSAGES, 'the raw intake keeps every message')
  assert.equal(result.inserted, 1, 'ONE warehouse row for one Person x one source')
  assert.equal(result.errors, 0)

  const land = kinds.filter((kind) => kind === 'land').length
  const latest = kinds.filter((kind) => kind === 'latest').length
  assert.equal(land, Math.ceil(MESSAGES / CHUNK), `landing is chunked, not ${MESSAGES} statements`)
  assert.equal(latest, 1, `expected one warehouse write, got ${latest}`)
  assert.ok(kinds.length < 20, `expected a handful of statements, got ${kinds.length}`)

  // The upsert must keep the newest message and never let an older one overwrite it.
  assert.ok(sql.latest?.includes('do update'), 'the warehouse write updates rather than appends')
  assert.ok(
    sql.latest?.includes('interaction.occurred_at < excluded.occurred_at'),
    'an older message can never overwrite a newer one',
  )
  assert.ok(
    sql.latest?.includes('on conflict (source_system, source_external_id)'),
    'the grain is the source, so each run touches the same row',
  )
})

test('a re-run lands nothing new and reports the source row as already current', async () => {
  const { execute, kinds } = fakeExecutor({ alreadyLanded: true, upsertReturnsRow: false })
  const result = await materializeAppleMessages(buildExport(), execute, { refresh: async () => {} })

  assert.equal(result.landed, 0, 'nothing new in the raw intake')
  assert.equal(result.inserted, 0)
  assert.equal(result.replayed, 1, 'the source row already carried the newest message')
  assert.ok(kinds.length < 20, `replay should not fan out per row, got ${kinds.length}`)
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
