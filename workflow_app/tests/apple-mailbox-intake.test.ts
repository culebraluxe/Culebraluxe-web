import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { IntakeCheckpoint, SaveIntakeCheckpointInput } from '../../db/intake-checkpoint'
import type { QueryExecutor, QueryRow } from '../../db/query-executor'
import {
  APPLE_PAGE_SIZE,
  parseAppleCursor,
  runAppleMailboxShard,
  serializeAppleCursor,
  type AppleLocalMailRecord,
  type AppleMailTransport,
} from '../../lib/intake/mailbox-paging'
import { landAppleRecord, parseRecords, runAppleVerify } from '../../scripts/apple-mailbox-intake'

// ---------------------------------------------------------------------------
// APPLE MAILBOX INTAKE — bounded extraction of the PROVEN exporter.
// The exporter never returns unbounded mailbox history: one bounded page is
// fetched, fully landed, checkpointed, and only then is the next page requested.
// No database, no Mail.app: the transport and the checkpoint store are faked.
// ---------------------------------------------------------------------------

const ACCOUNT = 'lisa@culebraluxe.com'

function record(overrides: Partial<AppleLocalMailRecord> = {}): AppleLocalMailRecord {
  return {
    mailbox: 'inbox',
    mailboxName: 'INBOX',
    localId: 42,
    messageId: '<abc@culebraluxe.com>',
    occurredAt: '2026-09-11T10:00:00.000Z',
    sender: 'Someone <someone@example.com>',
    to: [{ address: 'lisa@culebraluxe.com', name: 'Lisa' }],
    cc: [],
    bcc: [],
    subject: 'Hello',
    ...overrides,
  }
}

function checkpointStore() {
  const map = new Map<string, IntakeCheckpoint>()
  let failSave: Error | null = null
  return {
    map,
    failNextSave(error: Error) {
      failSave = error
    },
    load: async (shardKey: string): Promise<IntakeCheckpoint | null> => map.get(shardKey) ?? null,
    save: async (input: SaveIntakeCheckpointInput): Promise<void> => {
      if (failSave) {
        const error = failSave
        failSave = null
        throw error
      }
      map.set(input.shardKey, {
        ...input,
        shardStart: input.shardStart ?? null,
        shardEnd: input.shardEnd ?? null,
        updatedAt: '2026-09-11T00:00:00.000Z',
      })
    },
  }
}

/** Capture the SQL the landing writer sends, without a database. */
function captureExecutor(): { calls: Array<{ sql: string; values: unknown[] }>; execute: QueryExecutor } {
  const calls: Array<{ sql: string; values: unknown[] }> = []
  const execute = (async (strings: TemplateStringsArray, ...values: unknown[]) => {
    calls.push({ sql: strings.join(' ').replace(/\s+/g, ' '), values })
    return [{ id: 'row-1' }] as QueryRow[]
  }) as QueryExecutor
  return { calls, execute }
}

// --- 14. the exporter request is bounded ------------------------------------

test('applemail: a page is bounded, and an over-sized page is a contract violation', async () => {
  assert.equal(APPLE_PAGE_SIZE, 500)
  const store = checkpointStore()
  const transport: AppleMailTransport = {
    async fetchPage() {
      return { records: Array.from({ length: 3 }, (_, i) => record({ localId: i })), nextCursor: null, complete: false }
    },
  }
  await assert.rejects(
    runAppleMailboxShard(
      { transport, sourceAccount: ACCOUNT, land: async () => true, loadCheckpoint: store.load, saveCheckpoint: store.save, pageSize: 2 },
      'inbox',
    ),
    /acquisition must be bounded/,
  )
})

// --- 15. one page lands before the next page is requested -------------------

test('applemail: one page lands before the next page is requested', async () => {
  const events: string[] = []
  const store = checkpointStore()
  let call = 0
  const transport: AppleMailTransport = {
    async fetchPage(_kind, cursor) {
      call += 1
      events.push(`fetch:${call}:cursor=${cursor ? 'yes' : 'no'}`)
      if (call === 1) {
        return {
          records: [record({ localId: 1 }), record({ localId: 2 })],
          nextCursor: { index: 1, occurredAt: '2026-09-11T10:00:00.000Z', localId: 2 },
          complete: false,
        }
      }
      return { records: [record({ localId: 3 })], nextCursor: null, complete: true }
    },
  }
  const result = await runAppleMailboxShard(
    {
      transport,
      sourceAccount: ACCOUNT,
      land: async (r) => {
        events.push(`land:${r.localId}`)
        return true
      },
      loadCheckpoint: store.load,
      saveCheckpoint: store.save,
    },
    'inbox',
  )
  assert.equal(result.pages, 2)
  assert.equal(result.recordsLanded, 3)
  // The second fetch happened only after both page-1 records landed.
  assert.ok(events.indexOf('fetch:2:cursor=yes') > events.indexOf('land:1'))
  assert.ok(events.indexOf('fetch:2:cursor=yes') > events.indexOf('land:2'))
  assert.equal(store.map.get('mailbox:inbox')?.status, 'complete')
  assert.equal(store.map.get('mailbox:inbox')?.cursor, null)
})

// --- 16. the checkpoint advances only after a completed landing -------------

test('applemail: the checkpoint advances only after the page has landed', async () => {
  const events: string[] = []
  const store = checkpointStore()
  const transport: AppleMailTransport = {
    async fetchPage() {
      events.push('fetch')
      return { records: [record({ localId: 1 }), record({ localId: 2 })], nextCursor: null, complete: true }
    },
  }
  await runAppleMailboxShard(
    {
      transport,
      sourceAccount: ACCOUNT,
      land: async (r) => {
        events.push(`land:${r.localId}`)
        return true
      },
      loadCheckpoint: store.load,
      saveCheckpoint: async (input) => {
        events.push('checkpoint')
        await store.save(input)
      },
    },
    'inbox',
  )
  const checkpointIndex = events.indexOf('checkpoint')
  assert.ok(checkpointIndex > events.indexOf('land:1'))
  assert.ok(checkpointIndex > events.indexOf('land:2'))
  assert.equal(store.map.get('mailbox:inbox')?.recordsLanded, 2)
})

// --- 17. a crash before the checkpoint safely replays the page ---------------

test('applemail: a crash before the checkpoint replays the same page safely', async () => {
  const store = checkpointStore()
  const requests: Array<string | null> = []
  const transport: AppleMailTransport = {
    async fetchPage(_kind, cursor) {
      requests.push(cursor ? 'yes' : 'no')
      return { records: [record({ localId: 1 }), record({ localId: 2 })], nextCursor: null, complete: true }
    },
  }
  const landed = new Set<number>()
  const land = async (r: AppleLocalMailRecord) => !landed.has(r.localId) && (landed.add(r.localId), true)
  const deps = { transport, sourceAccount: ACCOUNT, land, loadCheckpoint: store.load, saveCheckpoint: store.save }

  store.failNextSave(new Error('process died before the checkpoint landed'))
  await assert.rejects(runAppleMailboxShard(deps, 'inbox'), /died before the checkpoint/)
  assert.equal(store.map.size, 0)

  const resumed = await runAppleMailboxShard(deps, 'inbox')
  assert.deepEqual(requests, ['no', 'no'], 'the same page was requested again')
  assert.equal(resumed.recordsLanded, 0)
  assert.equal(resumed.recordsReplayed, 2)
})


// --- 18, 19, 20. the replay identity: Message-ID, real local id, never index --

test('applemail: a real Message-ID is preferred as the replay identity', async () => {
  const { calls, execute } = captureExecutor()
  const inserted = await landAppleRecord(record({ messageId: '<abc@x>', localId: 99 }), ACCOUNT, execute)
  assert.equal(inserted, true)
  assert.ok(calls[0].values.includes('message-id:<abc@x>'))
})

test('applemail: a real stable local id is accepted when there is no Message-ID', async () => {
  const { calls, execute } = captureExecutor()
  await landAppleRecord(record({ messageId: null, localId: 4242 }), ACCOUNT, execute)
  assert.ok(calls[0].values.includes('mail-local:inbox:4242'))
})

test('applemail: an array position is never used as a message identity', async () => {
  const { execute } = captureExecutor()
  await assert.rejects(
    landAppleRecord(record({ messageId: null, localId: null }), ACCOUNT, execute),
    /unlandable Apple record/,
  )
  // The cursor is never fabricated from a position either.
  assert.equal(parseAppleCursor(null), null)
  assert.equal(parseAppleCursor('not json'), null)
  assert.equal(parseAppleCursor('{"index":"zero"}'), null)
  const cursor = { index: 5, occurredAt: '2026-09-11T10:00:00.000Z', localId: 7 }
  assert.deepEqual(parseAppleCursor(serializeAppleCursor(cursor)), cursor)
})

// --- 21. verify mode performs zero writes -----------------------------------

test('applemail: verify requests one bounded page and performs zero writes', async () => {
  let fetches = 0
  const transport: AppleMailTransport = {
    async fetchPage(kind) {
      fetches += 1
      return { records: [record({ mailbox: kind }), record({ mailbox: kind, localId: 2 })], nextCursor: null, complete: true }
    },
  }
  const result = await runAppleVerify(transport, 'inbox')
  assert.equal(fetches, 1, 'verify requests exactly one page')
  assert.equal(result.received, 2)
  assert.equal(result.complete, true)
  // Nothing here touches a landing or checkpoint writer by construction.
})

// --- exporter output parsing + null-safe local id ---------------------------

test('applemail: the exporter JSONL is parsed, and a missing local id survives as null', () => {
  const jsonl = [
    JSON.stringify(record({ localId: 1 })),
    '',
    JSON.stringify(record({ messageId: null, localId: null, occurredAt: null })),
  ].join('\n')
  const records = parseRecords(jsonl)
  assert.equal(records.length, 2)
  assert.equal(records[0].localId, 1)
  assert.equal(records[1].localId, null, 'the exporter never substitutes an index for a real id')
  assert.equal(records[1].messageId, null)
  assert.throws(() => parseRecords('{not json}'), /Invalid Mail.app metadata JSON/)
})
