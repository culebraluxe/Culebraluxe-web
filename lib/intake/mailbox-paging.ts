// ---------------------------------------------------------------------------
// INTAKE - bounded, resumable Mail.app header paging.
//
// This is the LOCAL-BOX path, and it is the one that works: Apple Mail is
// already signed in to every mailbox we care about (including the Gmail
// accounts), so Mail.app has the mail on disk and the JXA bridge
// (scripts/macbridge/apple-mail-metadata.jxa) reads header information out of
// it. There is no Gmail API, no OAuth client, no refresh token - none of that is
// needed to read headers off the local box.
//
// The only thing added on top of the 2026-09-02 reader that already worked is
// that it is now BOUNDED:
//
//   local box -> bounded page (500 records) -> land immediately -> checkpoint -> next
//
// So mailbox size is irrelevant, a run can be resumed, and a process may die at
// any point: the cursor advances ONLY after every record in a page has landed
// (or replayed), so a crash simply redoes that page and the l_applemail replay
// key absorbs the duplicates.
//
// Pure: no database, no child process, no SQL. The transport and the landing /
// checkpoint writers are injected by the intake script, which keeps the loop
// testable end-to-end without I/O.
// ---------------------------------------------------------------------------

import type { IntakeCheckpoint, SaveIntakeCheckpointInput } from '../../db/intake-checkpoint'

/** A page must never return unbounded history. */
export const APPLE_PAGE_SIZE = 500
export const APPLE_MAIL_INTAKE_SOURCE = 'applemail'

// --- the paging cursor ------------------------------------------------------

/**
 * The reader's real stable ordering key: occurred_at DESC, local id DESC.
 *
 * `index` is Mail.app's own enumeration position for the last emitted record and
 * is used ONLY as a fast-path resume hint: on the next page the reader checks
 * that position still holds that same record and, if it does, seeks straight to
 * it. If the mailbox moved (new mail prepended), the check fails and the reader
 * falls back to a linear scan for the first record not newer than the cursor.
 * The cursor is derived from the proven reader, never invented.
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
  sourceAccount: string
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

function skippedStats(sourceAccount: string, checkpoint: IntakeCheckpoint): MailboxRunStats {
  return {
    sourceAccount,
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

// --- the local-box record ---------------------------------------------------

/** One record from the proven JXA reader. Mirrors its JSON shape exactly. */
export type AppleLocalMailRecord = {
  mailbox: 'inbox' | 'sent' | string
  mailboxName: string
  localId: number | null
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
 * Walk ONE mailbox of ONE account, page by page.
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
    log(`mailbox [${deps.sourceAccount}] shard=${shardKey} COMPLETE - skipped`)
    return skippedStats(deps.sourceAccount, previous)
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
        `Mail.app reader returned ${page.records.length} records for one page (bound is ${pageSize}); acquisition must be bounded`,
      )
    }

    const outcomes: PageOutcome[] = []
    for (const record of page.records) {
      try {
        outcomes.push((await deps.land(record)) ? 'landed' : 'replayed')
      } catch (error) {
        log(
          `mailbox [${deps.sourceAccount}] record ${record.messageId ?? record.localId} failed: ${error instanceof Error ? error.message : String(error)}`,
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
      `mailbox [${deps.sourceAccount}] shard=${shardKey} page=${pages} received=${page.records.length} ` +
        `landed=${counted.landed} replayed=${counted.replayed} errors=${counted.errors} ${page.complete ? 'COMPLETE' : 'cursor=yes'}`,
    )

    if (page.complete) break
    if (!page.nextCursor) {
      throw new Error('Mail.app reader reported an incomplete page without a next cursor')
    }
    cursor = page.nextCursor
  }

  return {
    sourceAccount: deps.sourceAccount,
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
