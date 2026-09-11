import { test } from 'node:test'
import assert from 'node:assert/strict'

import type { IntakeCheckpoint, SaveIntakeCheckpointInput } from '../../db/intake-checkpoint'
import {
  GMAIL_MAX_RESULTS,
  GMAIL_METADATA_CONCURRENCY,
  GMAIL_METADATA_HEADERS,
  monthShards,
  runGmailShard,
  type DateShard,
  type GmailListPage,
  type GmailTransport,
} from '../../lib/intake/mailbox-paging'
import {
  DEFAULT_GMAIL_MAILBOX,
  assertGmailAccount,
  gmailListPath,
  gmailMetadataPath,
  gmailMetadataToLandedEmail,
} from '../../lib/intake/gmail'
import { selectShards } from '../../scripts/gmail-mailbox-intake'
import type { GmailMetadataMessage } from '../../lib/relationship-intel/gmail-latest-context'

// ---------------------------------------------------------------------------
// GMAIL MAILBOX INTAKE — bounded, resumable, replay-safe.
// Acceptance: the mailbox may hold 200,000 messages and the intake behaves
// exactly as if it held 200 - only the number of replay-safe pages changes. No
// database, no network: the transport and the checkpoint store are faked.
// ---------------------------------------------------------------------------

const ACCOUNT = DEFAULT_GMAIL_MAILBOX

function message(id: string, headers: Record<string, string> = {}, internalDate = '1700000000000'): GmailMetadataMessage {
  return {
    id,
    threadId: `thread-${id}`,
    internalDate,
    payload: { headers: Object.entries(headers).map(([name, value]) => ({ name, value })) },
  }
}

const SHARD: DateShard = {
  key: '2026-08',
  startIso: '2026-08-01T00:00:00.000Z',
  endIso: '2026-09-01T00:00:00.000Z',
}

function checkpointStore() {
  const map = new Map<string, IntakeCheckpoint>()
  let failSave: Error | null = null
  return {
    map,
    seed(checkpoint: IntakeCheckpoint) {
      map.set(checkpoint.shardKey, checkpoint)
    },
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
    cursor: () => map.get(SHARD.key)?.cursor ?? null,
  }
}

function seededCheckpoint(overrides: Partial<IntakeCheckpoint> = {}): IntakeCheckpoint {
  return {
    source: 'gmail',
    sourceAccount: ACCOUNT,
    shardKey: SHARD.key,
    shardStart: SHARD.startIso,
    shardEnd: SHARD.endIso,
    cursor: null,
    pagesCompleted: 0,
    recordsSeen: 0,
    recordsLanded: 0,
    recordsReplayed: 0,
    errors: 0,
    status: 'in_progress',
    updatedAt: '2026-09-11T00:00:00.000Z',
    ...overrides,
  }
}

// --- 1. the authenticated profile must be the scoped mailbox -----------------

test('gmail: the authenticated profile must equal penfield33@gmail.com', () => {
  assert.doesNotThrow(() => assertGmailAccount('Penfield33@Gmail.com', 'penfield33@gmail.com'))
  assert.throws(() => assertGmailAccount('someone@else.com', 'penfield33@gmail.com'), /Refusing to ingest another mailbox/)
  assert.throws(() => assertGmailAccount('', 'penfield33@gmail.com'), /did not report an email address/)
})

// --- 2. messages.list is bounded --------------------------------------------

test('gmail: messages.list uses maxResults <= 500 and never enumerates every id', () => {
  assert.equal(GMAIL_MAX_RESULTS, 500)
  const path = gmailListPath('after:2026/08/01 before:2026/09/01', null)
  const maxResults = Number(new URLSearchParams(path.split('?')[1]).get('maxResults'))
  assert.ok(maxResults <= 500, 'maxResults must stay bounded')
  assert.equal(maxResults, 500)
})

// --- 10. messages.get uses format=metadata ----------------------------------

test('gmail: messages.get uses format=metadata with the selected headers only', () => {
  const path = gmailMetadataPath('abc123')
  const params = new URLSearchParams(path.split('?')[1])
  assert.equal(params.get('format'), 'metadata')
  const headers = params.getAll('metadataHeaders')
  assert.deepEqual(headers, [...GMAIL_METADATA_HEADERS])
  assert.ok(headers.includes('From') && headers.includes('Subject') && headers.includes('Message-ID'))
})

// --- 11. no body / raw / full content is ever requested ---------------------

test('gmail: body, raw and full content are never requested', () => {
  const path = gmailMetadataPath('abc123').toLowerCase()
  for (const forbidden of ['format=full', 'format=raw', 'body', 'snippet', 'attachment', 'rfc822']) {
    assert.equal(path.includes(forbidden), false, `request must not ask for ${forbidden}`)
  }
})

// --- 12. ingestion does not require an existing CRM Person ------------------

test('gmail: a landed record carries no Person decision and no CRM identity', () => {
  const landed = gmailMetadataToLandedEmail(
    message('m1', { From: 'a@b.com', To: 'penfield33@gmail.com', Subject: 'Hello' }),
    ACCOUNT,
  )
  assert.ok(landed)
  assert.equal(landed.sourceAccount, ACCOUNT)
  assert.equal(landed.sourceMessageId, 'm1')
  assert.equal(landed.fromAddress, 'a@b.com')
  assert.equal(landed.subject, 'Hello')
  assert.equal('personId' in landed, false)
  assert.equal('canonicalPersonId' in landed, false)
  assert.equal(landed.bodyPreview, null)
  assert.deepEqual(landed.labels, null)
  // raw is the untouched metadata response
  assert.equal((landed.raw as GmailMetadataMessage).id, 'm1')
  // A message with no usable id is not landable.
  assert.equal(gmailMetadataToLandedEmail({ id: '   ' }, ACCOUNT), null)
})

// --- 3, 4. nextPageToken advances one page; rows land before the next page --

test('gmail: a page lands fully before the next page is requested', async () => {
  const events: string[] = []
  const store = checkpointStore()
  const pages: Record<string, GmailListPage> = {
    '': { ids: ['a', 'b'], nextPageToken: 'T2' },
    T2: { ids: ['c'], nextPageToken: null },
  }
  const transport: GmailTransport = {
    async listPage(_query, pageToken) {
      events.push(`list:${pageToken ?? 'first'}`)
      return pages[pageToken ?? '']
    },
    async getMetadata(id) {
      return message(id)
    },
  }

  const result = await runGmailShard(
    {
      transport,
      sourceAccount: ACCOUNT,
      land: async (m) => {
        events.push(`land:${m.id}`)
        return true
      },
      loadCheckpoint: store.load,
      saveCheckpoint: store.save,
    },
    SHARD,
  )

  assert.equal(result.pages, 2)
  assert.equal(result.recordsSeen, 3)
  assert.equal(result.recordsLanded, 3)
  assert.equal(result.status, 'complete')
  // Both page-1 rows landed before page 2 was requested.
  assert.ok(events.indexOf('list:T2') > events.indexOf('land:a'))
  assert.ok(events.indexOf('list:T2') > events.indexOf('land:b'))
  // Checkpoint advanced through both pages and completed the shard.
  assert.equal(store.cursor(), null)
  assert.equal(store.map.get(SHARD.key)?.status, 'complete')
  assert.equal(store.map.get(SHARD.key)?.pagesCompleted, 2)
})

// --- 5. same-page replay creates no duplicate -------------------------------

test('gmail: replaying the same page creates no duplicate', async () => {
  const store = checkpointStore()
  const landed = new Set<string>()
  const land = async (m: GmailMetadataMessage) => !landed.has(m.id) && (landed.add(m.id), true)
  const transport: GmailTransport = {
    async listPage() {
      return { ids: ['a', 'b'], nextPageToken: null }
    },
    async getMetadata(id) {
      return message(id)
    },
  }
  const deps = { transport, sourceAccount: ACCOUNT, land, loadCheckpoint: store.load, saveCheckpoint: store.save }

  const first = await runGmailShard(deps, SHARD)
  assert.equal(first.recordsLanded, 2)
  assert.equal(first.recordsReplayed, 0)

  // Re-run from the start of the page (the checkpoint was lost/never advanced).
  store.map.clear()
  const second = await runGmailShard(deps, SHARD)
  assert.equal(second.recordsLanded, 0, 'no duplicate rows are inserted')
  assert.equal(second.recordsReplayed, 2, 'the same page replays as already-landed')
})


// --- 6. crash before the checkpoint keeps the same page ---------------------

test('gmail: a crash before the checkpoint keeps the same page (and replays safely)', async () => {
  const store = checkpointStore()
  const requests: Array<string | null> = []
  const transport: GmailTransport = {
    async listPage(_query, pageToken) {
      requests.push(pageToken)
      return { ids: ['a', 'b'], nextPageToken: null }
    },
    async getMetadata(id) {
      return message(id)
    },
  }
  const landed = new Set<string>()
  const land = async (m: GmailMetadataMessage) => !landed.has(m.id) && (landed.add(m.id), true)
  const deps = { transport, sourceAccount: ACCOUNT, land, loadCheckpoint: store.load, saveCheckpoint: store.save }

  // Simulate the process dying exactly as the post-page checkpoint is written.
  store.failNextSave(new Error('process died before the checkpoint landed'))
  await assert.rejects(runGmailShard(deps, SHARD), /died before the checkpoint/)
  // Nothing was checkpointed, so the shard re-runs the SAME page.
  assert.equal(store.cursor(), null)
  assert.equal(store.map.size, 0)

  const resumed = await runGmailShard(deps, SHARD)
  assert.deepEqual(requests, [null, null], 'the same page was requested again')
  assert.equal(resumed.recordsReplayed, 2, 'the page replayed with no duplicate rows')
})

// --- 7. a completed page advances the checkpoint ----------------------------

test('gmail: a completed page advances the checkpoint cursor', async () => {
  const store = checkpointStore()
  const transport: GmailTransport = {
    async listPage(_query, pageToken) {
      return pageToken ? { ids: [], nextPageToken: null } : { ids: ['a'], nextPageToken: 'NEXT' }
    },
    async getMetadata(id) {
      return message(id)
    },
  }
  await runGmailShard(
    { transport, sourceAccount: ACCOUNT, land: async () => true, loadCheckpoint: store.load, saveCheckpoint: store.save },
    SHARD,
  )
  const saved = store.map.get(SHARD.key)
  assert.equal(saved?.pagesCompleted, 2)
  assert.equal(saved?.status, 'complete')
  assert.equal(saved?.recordsSeen, 1)
})

// --- 8. a completed shard advances to the next shard ------------------------

test('gmail: month shards are independent, newest first, and a complete shard is skipped', () => {
  const shards = monthShards({ fromIso: '2026-06-01T00:00:00.000Z', toIso: '2026-08-15T00:00:00.000Z' })
  assert.deepEqual(shards.map((s) => s.key), ['2026-08', '2026-07', '2026-06'])

  const selected = selectShards({ incremental: false, sinceIso: '2026-06-01', now: new Date('2026-08-15T00:00:00Z') })
  assert.deepEqual(selected.map((s) => s.key), ['2026-08', '2026-07', '2026-06'])
  // A single shard can be re-run alone.
  assert.deepEqual(
    selectShards({ incremental: false, sinceIso: '2026-06-01', now: new Date('2026-08-15T00:00:00Z'), shardKey: '2026-07' }).map((s) => s.key),
    ['2026-07'],
  )
  assert.throws(
    () => selectShards({ incremental: false, sinceIso: '2026-06-01', now: new Date('2026-08-15T00:00:00Z'), shardKey: '2011-01' }),
    /outside the requested range/,
  )
  // Incremental mode is a single recent overlap window.
  const incremental = selectShards({ incremental: true, sinceIso: '2011-06-01', now: new Date('2026-08-15T00:00:00Z') })
  assert.equal(incremental.length, 1)
  assert.equal(incremental[0].key, 'incremental')
})

test('gmail: a shard already marked COMPLETE is skipped', async () => {
  const store = checkpointStore()
  store.seed(seededCheckpoint({ status: 'complete', pagesCompleted: 3, recordsLanded: 7 }))
  let called = 0
  const transport: GmailTransport = {
    async listPage() {
      called += 1
      return { ids: [], nextPageToken: null }
    },
    async getMetadata(id) {
      return message(id)
    },
  }
  const result = await runGmailShard(
    { transport, sourceAccount: ACCOUNT, land: async () => true, loadCheckpoint: store.load, saveCheckpoint: store.save },
    SHARD,
  )
  assert.equal(called, 0, 'a complete shard makes no transport calls')
  assert.equal(result.skipped, true)
  assert.equal(result.recordsLanded, 7)
})


// --- 9. an invalid saved pageToken restarts the shard safely ----------------

test('gmail: a rejected saved pageToken restarts the current shard from its first page', async () => {
  const store = checkpointStore()
  store.seed(seededCheckpoint({ cursor: 'STALE-TOKEN', pagesCompleted: 4, recordsLanded: 40 }))
  const seen: Array<string | null> = []
  const transport: GmailTransport = {
    async listPage(_query, pageToken) {
      seen.push(pageToken)
      if (pageToken === 'STALE-TOKEN') throw new Error('Gmail API 400: Invalid page token')
      return { ids: ['a'], nextPageToken: null }
    },
    async getMetadata(id) {
      return message(id)
    },
  }
  const lines: string[] = []
  const result = await runGmailShard(
    {
      transport,
      sourceAccount: ACCOUNT,
      land: async () => true,
      loadCheckpoint: store.load,
      saveCheckpoint: store.save,
      logger: (line) => lines.push(line),
    },
    SHARD,
  )
  assert.deepEqual(seen, ['STALE-TOKEN', null])
  assert.ok(lines.some((line) => line.includes('restarting shard')))
  assert.equal(result.status, 'complete')
  // Counters accumulate across the restart rather than resetting the history.
  assert.equal(store.map.get(SHARD.key)?.recordsLanded, 41)
})

// --- 13. only bounded concurrency is used -----------------------------------

test('gmail: metadata gets use bounded concurrency, never 500 at once', async () => {
  assert.equal(GMAIL_METADATA_CONCURRENCY, 10)
  let inFlight = 0
  let peak = 0
  const ids = Array.from({ length: 100 }, (_, i) => `m${i}`)
  const store = checkpointStore()
  const transport: GmailTransport = {
    async listPage() {
      return { ids, nextPageToken: null }
    },
    async getMetadata(id) {
      inFlight += 1
      peak = Math.max(peak, inFlight)
      await new Promise((r) => setTimeout(r, 1))
      inFlight -= 1
      return message(id)
    },
  }
  await runGmailShard(
    { transport, sourceAccount: ACCOUNT, land: async () => true, loadCheckpoint: store.load, saveCheckpoint: store.save, concurrency: 5 },
    SHARD,
  )
  assert.ok(peak <= 5, `peak concurrency ${peak} must stay <= 5`)
  assert.ok(peak > 1, 'the work is actually concurrent')
})

// --- a failed page never advances the checkpoint ----------------------------

test('gmail: a failed page does not advance the checkpoint', async () => {
  const store = checkpointStore()
  let call = 0
  const transport: GmailTransport = {
    async listPage() {
      call += 1
      if (call === 1) return { ids: ['a'], nextPageToken: 'P2' }
      throw new Error('Gmail API 500: backend error')
    },
    async getMetadata(id) {
      return message(id)
    },
  }
  await assert.rejects(
    runGmailShard(
      { transport, sourceAccount: ACCOUNT, land: async () => true, loadCheckpoint: store.load, saveCheckpoint: store.save },
      SHARD,
    ),
    /Gmail API 500/,
  )
  // Page 1 was checkpointed; page 2 failed before its checkpoint, so its cursor
  // was never written and a re-run resumes page 2.
  assert.equal(store.cursor(), 'P2')
  assert.equal(store.map.get(SHARD.key)?.pagesCompleted, 1)
})
