/**
 * WHAT THE HARNESS ACTUALLY SPENT — read from the harness's own database.
 *
 * The storyboard run row has had `tokens_input`, `tokens_output`, `cost_usd` and
 * `cost_source` columns since migration 107/133, with readers for all of them (the
 * scorecard, the story-detail widget, the cost estimator). What was missing was the
 * WRITE: nothing ever populated them, so every run reported `unmeasured` and the
 * widgets fell back to an estimate. The 2026-09-12 scorecard commit says it plainly —
 * it "names exactly what does not exist".
 *
 * The measurement was never lost, it was never wired. OpenCode records every session's
 * tokens and vendor cost in its own SQLite database, so this module reads that.
 *
 * FAIL SOFT BY DESIGN. A missing database, a schema change, or a locked file returns
 * `null`, which the contract already means "unmeasured" — telemetry must never fail a
 * run. Returning null is not swallowing: the null IS the report, and the scorecard
 * counts unmeasured runs.
 */
import { DatabaseSync } from 'node:sqlite'
import { homedir } from 'node:os'
import { join } from 'node:path'

/** Default location of the harness's own store. */
export const OPENCODE_DB_PATH = join(homedir(), '.local', 'share', 'opencode', 'opencode.db')

export type HarnessUsage = {
  /** The session this run executed in, for traceability. */
  sessionId: string
  tokensInput: number
  tokensOutput: number
  /** Vendor-reported USD. Reserves `cost_usd`; the widget estimate lives elsewhere. */
  costUsd: number
}

export type SessionUsageRow = {
  id: string
  timeCreated: number
  tokensInput: unknown
  tokensOutput: unknown
  cost: unknown
}

/**
 * Which session belongs to this run.
 *
 * The adapter stamps `externalRunId = opencode-<Date.now()>` at launch and never learns
 * the harness's session id, so the join is by TIME. Two rules, both load-bearing:
 *
 *   1. A session that started BEFORE this run launched is never this run's. Without this,
 *      a model-free lane (the deterministic Assay) claims the previous lane's session and
 *      double-counts its spend — observed live on 2026-09-13.
 *   2. Among the sessions that started after the launch, the EARLIEST is this run's: a
 *      following lane's session opens later, so it can never be stolen backwards.
 *
 * Pure: the caller supplies the rows, so this is testable without a database.
 */
export function pickSessionForRun(
  sessions: readonly SessionUsageRow[],
  harnessStartedAtMs: number,
  windowMs = 5 * 60_000,
  slackMs = 5_000,
): SessionUsageRow | null {
  let best: SessionUsageRow | null = null
  for (const session of sessions) {
    // Rule 1, with a small clock slack: the harness and the database can disagree by
    // a second or two about when the session began.
    if (session.timeCreated < harnessStartedAtMs - slackMs) continue
    if (session.timeCreated > harnessStartedAtMs + windowMs) continue
    // Rule 2: the first session opened by this launch, not a later lane's.
    if (!best || session.timeCreated < best.timeCreated) best = session
  }
  return best
}

/** Coerce a driver value; anything not a finite number is 0. */
function count(value: unknown): number {
  const n = Number(value ?? 0)
  return Number.isFinite(n) ? n : 0
}

/** The harness's usage for a run that started at `harnessStartedAtMs`, or null. */
export function readHarnessUsage(input: {
  harnessStartedAtMs: number
  dbPath?: string
  windowMs?: number
}): HarnessUsage | null {
  const windowMs = input.windowMs ?? 5 * 60_000
  const dbPath = input.dbPath ?? OPENCODE_DB_PATH
  let db: DatabaseSync | null = null
  try {
    db = new DatabaseSync(dbPath, { readOnly: true })
    const rows = db
      .prepare(
        'select id, time_created, tokens_input, tokens_output, cost from session ' +
          'where time_created between ? and ?',
      )
      .all(
        input.harnessStartedAtMs - windowMs,
        input.harnessStartedAtMs + windowMs,
      ) as Array<Record<string, unknown>>
    const sessions: SessionUsageRow[] = rows.map((row) => ({
      id: String(row.id),
      timeCreated: count(row.time_created),
      tokensInput: row.tokens_input,
      tokensOutput: row.tokens_output,
      cost: row.cost,
    }))
    const session = pickSessionForRun(sessions, input.harnessStartedAtMs, windowMs)
    if (!session) return null
    return {
      sessionId: session.id,
      tokensInput: count(session.tokensInput),
      tokensOutput: count(session.tokensOutput),
      costUsd: count(session.cost),
    }
  } catch {
    // Unmeasured, explicitly. See the header: telemetry never fails a run.
    return null
  } finally {
    try {
      db?.close()
    } catch {
      /* closing a read-only handle cannot matter */
    }
  }
}

/** The launch instant of a run, from the harness id or the work item's start time. */
export function harnessStartedAtMs(input: {
  externalRunId?: string | null
  startedAt?: string | null
}): number | null {
  const stamp = /^opencode-(\d+)$/.exec(input.externalRunId ?? '')
  if (stamp) {
    const ms = Number(stamp[1])
    if (Number.isFinite(ms) && ms > 0) return ms
  }
  const parsed = input.startedAt ? Date.parse(input.startedAt) : Number.NaN
  return Number.isFinite(parsed) ? parsed : null
}
