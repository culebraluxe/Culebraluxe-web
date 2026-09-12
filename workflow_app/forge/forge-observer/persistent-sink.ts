// ---------------------------------------------------------------------------
// Durable TraceSink: the in-memory sink, written through to the EXISTING Forge
// trace (workflow_execution_trace_event via db/workflow-trace.recordTraceEvent).
//
// Why an adapter rather than a new store: the trace table already owns command/
// domain-level history (COMMAND_RECEIVED/COMPLETED/FAILED, DOMAIN_EVENT_EMITTED)
// keyed by workflow instance/node/task. The observer adds the worker-EXECUTION
// layer that nothing recorded before. Two stores would be two histories; one
// table per fact is the rule (docs/REAL-ESTATE-TRANSACTION-DESIGN.md section 9,
// and the Maestro review's "do not add a second source").
//
// list() stays synchronous because the contract is synchronous; the memory sink
// is the process-local view and the durable write is a side effect. The write
// NEVER throws or blocks a run: recordTraceEvent already contains its own
// failures, and a recorder that can fail a run is not an observer.
// ---------------------------------------------------------------------------

import { randomUUID } from 'node:crypto'

import type { TraceEvent, TraceSink } from './types'
import { createMemoryTraceSink } from './sink'
import { traceEventsFromRows, type PersistedTraceRow } from './rehydrate'

/** The shape recordTraceEvent consumes (kept structural so tests need no DB). */
export type TraceWrite = (input: {
  eventType: string
  system: string
  occurredAt: string
  traceId: string
  correlationId: string | null
  workflowInstanceId: string | null
  workflowNodeId: string | null
  taskId: string | null
  outcome: string | null
  summary: string | null
  metadata: Record<string, string | number | boolean | null>
  sourceSystem: string
  sourceEventId: string
}) => Promise<void>

export type PersistentSinkOptions = {
  write: TraceWrite
  /**
   * FORGE-OBS-LIST-01 — read a story's persisted history back in. Optional so a
   * caller with no reader (tests, observer-mode) keeps the old behaviour: the
   * sink simply stays process-local.
   */
  read?: TraceRead
  /** Root correlation for every event this process records. */
  traceId?: string
  /** Defaults to 'forge_observer'. */
  sourceSystem?: string
}

/** Where a story's history can be reread from, scoped to one story. */
export type TraceRead = (key: {
  storyId: string
  processInstanceId: string
}) => Promise<PersistedTraceRow[]>

export function createPersistentTraceSink(
  options: PersistentSinkOptions,
): TraceSink & {
  snapshot(storyId?: string): TraceEvent[]
  load(key: { storyId: string; processInstanceId: string }): Promise<number>
} {
  const memory = createMemoryTraceSink()
  const traceId = options.traceId ?? randomUUID()
  const sourceSystem = options.sourceSystem ?? 'forge_observer'
  const loaded = new Set<string>()

  return {
    append(partial) {
      const event = memory.append(partial)
      // Deterministic source identity => replay-safe de-dupe in the trace table.
      const sourceEventId = [event.storyId, event.taskId, event.nodeId, event.attempt, event.seq].join(':')
      void options
        .write({
          // The trace's convention is underscore event types (COMMAND_RECEIVED);
          // observer kinds are dotted (scope.check), so normalize.
          eventType: event.kind.replace(/\./g, '_').toUpperCase(),
          system: sourceSystem,
          occurredAt: event.atIso,
          traceId,
          correlationId: event.processInstanceId ?? null,
          workflowInstanceId: event.processInstanceId ?? null,
          workflowNodeId: event.nodeId ?? null,
          taskId: event.taskId ?? null,
          outcome: event.verdict ?? null,
          summary: event.reason ?? null,
          metadata: {
            attempt: event.attempt,
            storyId: event.storyId,
            ...(event.sha ? { sha: event.sha } : {}),
            ...(event.paths?.length ? { paths: event.paths.join(',') } : {}),
            ...(event.detail ?? {}),
          },
          sourceSystem,
          sourceEventId,
        })
        .catch(() => {
          // Observer only: a failed write must never fail a run.
        })
      return event
    },
    list: memory.list,
    snapshot: memory.snapshot,
    /**
     * FORGE-OBS-LIST-01 — the one call that makes an attempt survive a restart.
     *
     * Once per story per process: attempt 2 does not re-read what attempt 1 of the
     * same process already appended. Never throws — a reader outage leaves the sink
     * process-local, which is exactly what it was before, and never breaks a run.
     * Returns how many events were taken in, so a caller (or a test) can tell an
     * empty history from a failed read only by its own logs, not by a throw.
     */
    async load(key) {
      if (!options.read) return 0
      if (loaded.has(key.storyId)) return 0
      loaded.add(key.storyId)
      try {
        const rows = await options.read(key)
        const events = traceEventsFromRows(rows, key.storyId)
        // Oldest-first: the reader orders by occurred_at, and seq ordering is what
        // every alert rule compares against.
        events.sort((a, b) => a.seq - b.seq)
        memory.seed(events)
        return events.length
      } catch {
        // Observer only. Silence here is deliberate: this is the recorder's own
        // read, and a run must not depend on it.
        loaded.delete(key.storyId)
        return 0
      }
    },
  }
}
