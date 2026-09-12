// ---------------------------------------------------------------------------
// FORGE-OBS-LIST-01 — read a story's history BACK into the observer's sink.
//
// The sink's list() is process-local: it is the memory sink, and the durable
// write is a side effect. That is fine until the process it describes is not the
// process that recorded it. After a worker restart the sink starts empty, so
//   * RETRY_UNCHANGED_INPUT cannot see the attempt that ran before the restart
//     (it reports repeated no-op retries only within one process lifetime), and
//   * drainAlerts re-records every alert as if it were new, because the de-dupe
//     set it reads (sink.list) is empty too — phantom alerts after any restart.
//
// This module is the reverse of the write path in persistent-sink.ts: rows of
// workflow_execution_trace_event back into observer TraceEvents. Unknown rows are
// SKIPPED rather than guessed: the same read returns runtime events
// (COMMAND_RECEIVED and friends), and inventing a kind for those would corrupt
// every rule that reads the trace.
//
// Nothing here throws and nothing here gates. A history that cannot be read
// leaves the sink as it was: dark, exactly as it is today.
// ---------------------------------------------------------------------------

import type { TraceEvent, TraceEventKind, TraceVerdict } from './types'

/** Structural shape of a persisted trace row (a superset is fine). */
export type PersistedTraceRow = {
  eventType?: string | null
  occurredAt?: string | null
  outcome?: string | null
  summary?: string | null
  metadata?: Record<string, unknown> | null
  sourceSystem?: string | null
  sourceEventId?: string | null
  workflowInstanceId?: string | null
  workflowNodeId?: string | null
  taskId?: string | null
}

/** The observer's event kinds, as a runtime set so unknown rows can be skipped. */
const KINDS: ReadonlySet<string> = new Set<TraceEventKind>([
  'run.start',
  'tool.intend',
  'tool.result',
  'fs.write',
  'git.commit',
  'scope.check',
  'run.end',
  'hold',
  'alert',
])

const VERDICTS: ReadonlySet<string> = new Set<TraceVerdict>(['allow', 'deny', 'watch'])

/** Keys the write path lifts OUT of detail and into named columns. */
const RESERVED = new Set(['attempt', 'storyId', 'sha', 'paths'])

/**
 * `run.start` -> `run.start`. The writer uppercases and underscores the kind;
 * this is the inverse. A kind this build does not know is not an error: it is a
 * row written by a different version of the observer, and it is skipped.
 */
export function kindOf(eventType: string | null | undefined): TraceEventKind | null {
  if (!eventType) return null
  const kind = eventType.trim().toLowerCase().replace(/_/g, '.')
  return KINDS.has(kind) ? (kind as TraceEventKind) : null
}

/**
 * `seq` and `attempt` are recoverable from sourceEventId
 * (`storyId:taskId:nodeId:attempt:seq`), and recovering them matters: the de-dupe
 * key is built from them, so a rehydrated event must carry the SAME numbers it
 * was written with or the next write would collide with it and vanish.
 */
function fromSourceEventId(sourceEventId: string | null | undefined): {
  attempt: number | null
  seq: number | null
} {
  if (!sourceEventId) return { attempt: null, seq: null }
  const parts = sourceEventId.split(':')
  if (parts.length < 5) return { attempt: null, seq: null }
  const seq = Number(parts[parts.length - 1])
  const attempt = Number(parts[parts.length - 2])
  return {
    attempt: Number.isInteger(attempt) && attempt >= 1 ? attempt : null,
    seq: Number.isInteger(seq) && seq >= 1 ? seq : null,
  }
}

function text(value: unknown): string | null {
  if (value == null) return null
  const s = String(value).trim()
  return s === '' ? null : s
}

/**
 * Rows -> observer events, oldest first, for ONE story.
 *
 * `storyId` is required and is enforced here: the caller's read is scoped to a
 * workflow instance, and an instance can outlive one story's trace, so the
 * metadata storyId is the filter that keeps another story's history out of this
 * story's alert evaluation.
 */
export function traceEventsFromRows(rows: PersistedTraceRow[], storyId: string): TraceEvent[] {
  const events: TraceEvent[] = []
  let position = 0
  for (const row of rows) {
    const kind = kindOf(row.eventType)
    if (!kind) continue
    const metadata = row.metadata ?? {}
    if (text(metadata.storyId) !== storyId) continue
    position += 1
    const fromId = fromSourceEventId(row.sourceEventId)
    const attempt = Number(metadata.attempt)
    const paths = text(metadata.paths)
    const verdict = text(row.outcome)
    const detail: Record<string, string | number | boolean | null> = {}
    for (const [key, value] of Object.entries(metadata)) {
      if (RESERVED.has(key)) continue
      if (value === null || ['string', 'number', 'boolean'].includes(typeof value)) {
        detail[key] = value as string | number | boolean | null
      }
    }
    events.push({
      seq: fromId.seq ?? position,
      atIso: text(row.occurredAt) ?? new Date().toISOString(),
      kind,
      storyId,
      processInstanceId: text(row.workflowInstanceId) ?? '',
      taskId: text(row.taskId) ?? '',
      nodeId: text(row.workflowNodeId) ?? '',
      attempt: fromId.attempt ?? (Number.isInteger(attempt) && attempt >= 1 ? attempt : 1),
      // Provenance hints the write path did not persist. They are not identity:
      // nothing keys on them, and a rehydrated event says so honestly instead of
      // inventing a worktree it cannot know.
      worktreePath: '',
      baseCommit: '',
      ...(text(metadata.sha) ? { sha: text(metadata.sha)! } : {}),
      ...(paths ? { paths: paths.split(',').map((p) => p.trim()).filter(Boolean) } : {}),
      ...(verdict && VERDICTS.has(verdict) ? { verdict: verdict as TraceVerdict } : {}),
      ...(text(row.summary) ? { reason: text(row.summary)! } : {}),
      ...(Object.keys(detail).length ? { detail } : {}),
    })
  }
  return events
}
