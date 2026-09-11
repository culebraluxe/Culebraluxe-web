// ---------------------------------------------------------------------------
// INTAKE — bounded, resumable mailbox paging.
//
// Makes mailbox SIZE irrelevant. Each source is pulled as a sequence of bounded
// pages, and every page follows the same contract:
//
//   fetch one bounded page -> land every record immediately -> checkpoint -> next
//
// A process may die at any point. The checkpoint cursor advances ONLY after every
// record in a page has landed (or replayed); a crash before that leaves the
// previous cursor in place, so the page is simply redone and the L-table replay
// key absorbs the duplicates. There is no "giant mailbox job" that must survive.
//
// This module is deliberately pure: no database, no fetch, no child process, no
// SQL. The transports and the landing/checkpoint writers are injected by the
// scripts, which keeps the loop testable end-to-end without I/O.
//
// HARD ACCEPTANCE CRITERION: no one-shot arrays. There is no shape like
//   const all = []; while (nextPage) { all.push(...page) }; await land(all)
// anywhere here - a page's records are landed and then released before the next
// page is requested.
// ---------------------------------------------------------------------------

import type { IntakeCheckpoint, SaveIntakeCheckpointInput } from '../../db/intake-checkpoint'
import type { GmailMetadataMessage } from '../relationship-intel/gmail-latest-context'
import { mapLimit } from '../relationship-intel/inmemory-lookup'

// --- shared limits ----------------------------------------------------------

/** Gmail REST maximum messages.list page size. Never ask for every message id. */
export const GMAIL_MAX_RESULTS = 500
/** Bounded concurrency for messages.get - never 500 simultaneous requests. */
export const GMAIL_METADATA_CONCURRENCY = 10
/** Apple exporter page size - a page must never return unbounded history. */
export const APPLE_PAGE_SIZE = 500
/** The only Gmail metadata headers we ask for. No bodies, snippets or MIME. */
export const GMAIL_METADATA_HEADERS = [
  'From',
  'To',
  'Cc',
  'Bcc',
  'Subject',
  'Message-ID',
  'Reply-To',
] as const

export const GMAIL_INTAKE_SOURCE = 'gmail'
export const APPLE_MAIL_INTAKE_SOURCE = 'applemail'

// --- date shards ------------------------------------------------------------

export type DateShard = {
  /** stable identifier, e.g. a Gmail 'YYYY-MM' month */
  key: string
  startIso: string
  endIso: string
}

function monthKey(date: Date): string {
  return `${date.getUTCFullYear()}-${String(date.getUTCMonth() + 1).padStart(2, '0')}`
}

/**
 * Independent, replay-safe calendar-month shards, newest -> oldest.
 *
 * Newest first is preferable: recent CRM context becomes available first, and
 * each shard is small enough to be abandoned/restarted without consequence.
 */
export function monthShards(opts: { fromIso: string; toIso: string }): DateShard[] {
  const from = new Date(opts.fromIso)
  const to = new Date(opts.toIso)
  if (Number.isNaN(from.getTime()) || Number.isNaN(to.getTime())) {
    throw new Error('monthShards requires valid ISO dates')
  }
  const floor = new Date(Date.UTC(from.getUTCFullYear(), from.getUTCMonth(), 1))
  let cursor = new Date(Date.UTC(to.getUTCFullYear(), to.getUTCMonth(), 1))
  const shards: DateShard[] = []
  while (cursor.getTime() >= floor.getTime()) {
    shards.push({
      key: monthKey(cursor),
      startIso: cursor.toISOString(),
      endIso: new Date(Date.UTC(cursor.getUTCFullYear(), cursor.getUTCMonth() + 1, 1)).toISOString(),
    })
    cursor = new Date(Date.UTC(cursor.getUTCFullYear(), cursor.getUTCMonth() - 1, 1))
  }
  return shards
}

/** The single shard used by incremental mode: a recent, intentionally overlapping window. */
export function recentWindowShard(days: number, now: Date = new Date()): DateShard {
  if (!Number.isFinite(days) || days <= 0) throw new Error('recentWindowShard requires a positive day count')
  return {
    key: 'incremental',
    startIso: new Date(now.getTime() - days * 86_400_000).toISOString(),
    endIso: now.toISOString(),
  }
}

function gmailDate(iso: string): string {
  const date = new Date(iso)
  return `${date.getUTCFullYear()}/${String(date.getUTCMonth() + 1).padStart(2, '0')}/${String(date.getUTCDate()).padStart(2, '0')}`
}

/** The proven Gmail historical query shape: a calendar window, trash/spam/drafts excluded. */
export function gmailShardQuery(shard: DateShard): string {
  return `after:${gmailDate(shard.startIso)} before:${gmailDate(shard.endIso)} -in:trash -in:spam -label:drafts`
}

// --- Apple paging cursor ----------------------------------------------------

/**
 * The Apple exporter's real stable ordering key: occurred_at DESC, local id DESC.
 *
 * `index` is Mail.app's own enumeration position for the last emitted record and
 * is used ONLY as a fast-path resume hint: on the next page the exporter checks
 * that position still holds that same record and, if it does, seeks straight to
 * it. If the mailbox moved (new mail prepended), the check fails and the exporter
 * falls back to a linear scan for the first record not newer than the cursor.
 * The cursor is derived from the proven exporter, never invented.
 */
export type AppleMailCursor = {
  index: number
  occurredAt: string | null
  localId: number | null
}

export function serializeAppleCursor(cursor: AppleMailCursor): string {
  return JSON.stringify(cursor)
}

export function parseAppleCursor(raw: string | null | undefined): AppleMailCursor | null {
  if (!raw) return null
  try {
    const parsed = JSON.parse(raw) as Partial<AppleMailCursor>
    if (typeof parsed.index !== 'number' || !Number.isFinite(parsed.index)) return null
    return {
      index: parsed.index,
      occurredAt: typeof parsed.occurredAt === 'string' ? parsed.occurredAt : null,
      localId: typeof parsed.localId === 'number' && Number.isFinite(parsed.localId) ? parsed.localId : null,
    }
  } catch {
    return null
  }
}

// --- per-page counters ------------------------------------------------------

export type PageOutcome = 'landed' | 'replayed' | 'error'

export type MailboxRunStats = {
  shardKey: string
  pages: number
  recordsSeen: number
  recordsLanded: number
  recordsReplayed: number
  errors: number
  status: 'in_progress' | 'complete'
  /** true when the shard was already complete and no work was done */
  skipped: boolean
}

function countOutcomes(outcomes: PageOutcome[]): {
  landed: number
  replayed: number
  errors: number
} {
  let landed = 0
  let replayed = 0
  let errors = 0
  for (const outcome of outcomes) {
    if (outcome === 'landed') landed += 1
    else if (outcome === 'replayed') replayed += 1
    else errors += 1
  }
  return { landed, replayed, errors }
}

function skippedStats(checkpoint: IntakeCheckpoint): MailboxRunStats {
  return {
    shardKey: checkpoint.shardKey,
    pages: checkpoint.pagesCompleted,
    recordsSeen: checkpoint.recordsSeen,
    recordsLanded: checkpoint.recordsLanded,
    recordsReplayed: checkpoint.recordsReplayed,
    errors: checkpoint.errors,
    status: 'complete',
    skipped: true,
  }
}

// --- Gmail ------------------------------------------------------------------

export type GmailListPage = {
  ids: string[]
  nextPageToken: string | null
}

export type GmailTransport = {
  /** messages.list for one bounded page. MUST use maxResults <= 500. */
  listPage(query: string, pageToken: string | null): Promise<GmailListPage>
  /** messages.get with format=metadata only. */
  getMetadata(id: string): Promise<GmailMetadataMessage>
}

export type GmailShardDeps = {
  transport: GmailTransport
  sourceAccount: string
  /** Land one metadata record immediately. true = inserted, false = replayed. */
  land: (message: GmailMetadataMessage) => Promise<boolean>
  loadCheckpoint: (shardKey: string) => Promise<IntakeCheckpoint | null>
  saveCheckpoint: (checkpoint: SaveIntakeCheckpointInput) => Promise<void>
  concurrency?: number
  logger?: (line: string) => void
  /** Recognizes a rejected persisted pageToken so the shard can safely restart. */
  isInvalidPageToken?: (error: unknown) => boolean
}

/** Default recognition of a Gmail "saved page token is no longer valid" response. */
export function defaultIsInvalidPageToken(error: unknown): boolean {
  const message = error instanceof Error ? error.message : String(error)
  return /Gmail API (400|404)/.test(message) || /invalid.*page ?token/i.test(message)
}

/**
 * Walk ONE date shard page by page.
 *
 * A page-level failure (the list call throwing) propagates and NO checkpoint is
 * written for that page, so a re-run resumes the same page. An individual message
 * failure is reported with its source id and the page continues. If a persisted
 * pageToken is rejected, this shard restarts from its first page rather than
 * failing the whole historical load.
 */
export async function runGmailShard(
  deps: GmailShardDeps,
  shard: DateShard,
): Promise<MailboxRunStats> {
  const log = deps.logger ?? (() => {})
  const concurrency = deps.concurrency ?? GMAIL_METADATA_CONCURRENCY
  const isInvalid = deps.isInvalidPageToken ?? defaultIsInvalidPageToken

  const previous = await deps.loadCheckpoint(shard.key)
  if (previous?.status === 'complete') {
    log(`gmail [${deps.sourceAccount}] shard=${shard.key} COMPLETE - skipped`)
    return skippedStats(previous)
  }

  const query = gmailShardQuery(shard)
  let pageToken: string | null = previous?.cursor ?? null
  let pages = previous?.pagesCompleted ?? 0
  let recordsSeen = previous?.recordsSeen ?? 0
  let recordsLanded = previous?.recordsLanded ?? 0
  let recordsReplayed = previous?.recordsReplayed ?? 0
  let errors = previous?.errors ?? 0

  for (;;) {
    let listed: GmailListPage
    try {
      listed = await deps.transport.listPage(query, pageToken)
    } catch (error) {
      if (pageToken && isInvalid(error)) {
        log(
          `gmail [${deps.sourceAccount}] shard=${shard.key} saved cursor rejected - restarting shard from its first page`,
        )
        pageToken = null
        continue
      }
      throw error
    }

    // Bounded concurrency, and each record lands as soon as its metadata is
    // obtained - we never wait for the whole page before any row is durable.
    const outcomes = await mapLimit(listed.ids, concurrency, async (id): Promise<PageOutcome> => {
      try {
        const message = await deps.transport.getMetadata(id)
        return (await deps.land(message)) ? 'landed' : 'replayed'
      } catch (error) {
        log(
          `gmail [${deps.sourceAccount}] message ${id} failed: ${error instanceof Error ? error.message : String(error)}`,
        )
        return 'error'
      }
    })

    const counted = countOutcomes(outcomes)
    pages += 1
    recordsSeen += listed.ids.length
    recordsLanded += counted.landed
    recordsReplayed += counted.replayed
    errors += counted.errors

    const next = listed.nextPageToken ?? null
    // Checkpoint advances ONLY now that every message in this page has landed or
    // replayed. A crash before this point simply redoes the page.
    await deps.saveCheckpoint({
      source: GMAIL_INTAKE_SOURCE,
      sourceAccount: deps.sourceAccount,
      shardKey: shard.key,
      shardStart: shard.startIso,
      shardEnd: shard.endIso,
      cursor: next,
      pagesCompleted: pages,
      recordsSeen,
      recordsLanded,
      recordsReplayed,
      errors,
      status: next ? 'in_progress' : 'complete',
    })

    log(
      `gmail [${deps.sourceAccount}] shard=${shard.key} page=${pages} received=${listed.ids.length} ` +
        `landed=${counted.landed} replayed=${counted.replayed} errors=${counted.errors} next=${next ? 'yes' : 'no'}`,
    )

    if (!next) break
    pageToken = next
  }

  return {
    shardKey: shard.key,
    pages,
    recordsSeen,
    recordsLanded,
    recordsReplayed,
    errors,
    status: 'complete',
    skipped: false,
  }
}

// --- Apple ------------------------------------------------------------------

/** One Apple exporter record. Mirrors the proven JXA record shape exactly. */
export type AppleLocalMailRecord = {
  mailbox: 'inbox' | 'sent' | string
  mailboxName: string
  localId: number
  messageId: string | null
  occurredAt: string | null
  sender: string | null
  to: Array<{ address: string | null; name: string | null }>
  cc: Array<{ address: string | null; name: string | null }>
  bcc: Array<{ address: string | null; name: string | null }>
  subject: string | null
}

export type AppleMailboxKind = 'inbox' | 'sent'

/**
 * A transport result. A call must NEVER return unbounded mailbox history:
 * `records` is at most one page, and `complete` says whether the mailbox is
 * exhausted (no further cursor).
 */
export type AppleMailboxPage = {
  records: AppleLocalMailRecord[]
  nextCursor: AppleMailCursor | null
  complete: boolean
}

export type AppleMailTransport = {
  fetchPage(mailboxKind: AppleMailboxKind, cursor: AppleMailCursor | null): Promise<AppleMailboxPage>
}

export type AppleMailShardDeps = {
  transport: AppleMailTransport
  sourceAccount: string
  /** Land one source record immediately. true = inserted, false = replayed. */
  land: (record: AppleLocalMailRecord) => Promise<boolean>
  loadCheckpoint: (shardKey: string) => Promise<IntakeCheckpoint | null>
  saveCheckpoint: (checkpoint: SaveIntakeCheckpointInput) => Promise<void>
  /** Hard bound on a single page; a transport exceeding it is a contract violation. */
  pageSize?: number
  logger?: (line: string) => void
}

/**
 * Walk ONE Apple mailbox page by page.
 *
 * A page is fetched, fully landed, and only then is the cursor checkpointed and
 * the next page requested. A crash mid-page restarts that page; the l_applemail
 * replay key absorbs the duplicates.
 */
export async function runAppleMailboxShard(
  deps: AppleMailShardDeps,
  mailboxKind: AppleMailboxKind,
): Promise<MailboxRunStats> {
  const log = deps.logger ?? (() => {})
  const pageSize = deps.pageSize ?? APPLE_PAGE_SIZE
  const shardKey = `mailbox:${mailboxKind}`

  const previous = await deps.loadCheckpoint(shardKey)
  if (previous?.status === 'complete') {
    log(`applemail [${deps.sourceAccount}] shard=${shardKey} COMPLETE - skipped`)
    return skippedStats(previous)
  }

  let cursor = parseAppleCursor(previous?.cursor ?? null)
  let pages = previous?.pagesCompleted ?? 0
  let recordsSeen = previous?.recordsSeen ?? 0
  let recordsLanded = previous?.recordsLanded ?? 0
  let recordsReplayed = previous?.recordsReplayed ?? 0
  let errors = previous?.errors ?? 0

  for (;;) {
    // One bounded page. If this throws, no checkpoint is written and the same
    // page is retried on the next run.
    const page = await deps.transport.fetchPage(mailboxKind, cursor)
    if (page.records.length > pageSize) {
      throw new Error(
        `Apple exporter returned ${page.records.length} records for one page (bound is ${pageSize}); acquisition must be bounded`,
      )
    }

    const outcomes: PageOutcome[] = []
    for (const record of page.records) {
      try {
        outcomes.push((await deps.land(record)) ? 'landed' : 'replayed')
      } catch (error) {
        log(
          `applemail [${deps.sourceAccount}] record ${record.messageId ?? record.localId} failed: ${error instanceof Error ? error.message : String(error)}`,
        )
        outcomes.push('error')
      }
    }

    const counted = countOutcomes(outcomes)
    pages += 1
    recordsSeen += page.records.length
    recordsLanded += counted.landed
    recordsReplayed += counted.replayed
    errors += counted.errors

    // Checkpoint advances ONLY after this page has fully landed.
    const status = page.complete ? 'complete' : 'in_progress'
    await deps.saveCheckpoint({
      source: APPLE_MAIL_INTAKE_SOURCE,
      sourceAccount: deps.sourceAccount,
      shardKey,
      shardStart: null,
      shardEnd: null,
      cursor: page.complete || !page.nextCursor ? null : serializeAppleCursor(page.nextCursor),
      pagesCompleted: pages,
      recordsSeen,
      recordsLanded,
      recordsReplayed,
      errors,
      status,
    })

    log(
      `applemail [${deps.sourceAccount}] shard=${shardKey} page=${pages} received=${page.records.length} ` +
        `landed=${counted.landed} replayed=${counted.replayed} errors=${counted.errors} cursor=${page.complete ? 'complete' : 'yes'}`,
    )

    if (page.complete) break
    if (!page.nextCursor) {
      throw new Error('Apple exporter reported an incomplete page without a next cursor')
    }
    cursor = page.nextCursor
  }

  return {
    shardKey,
    pages,
    recordsSeen,
    recordsLanded,
    recordsReplayed,
    errors,
    status: 'complete',
    skipped: false,
  }
}
