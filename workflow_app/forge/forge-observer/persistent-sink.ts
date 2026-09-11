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
  /** Root correlation for every event this process records. */
  traceId?: string
  /** Defaults to 'forge_observer'. */
  sourceSystem?: string
}

export function createPersistentTraceSink(
  options: PersistentSinkOptions,
): TraceSink & { snapshot(storyId?: string): TraceEvent[] } {
  const memory = createMemoryTraceSink()
  const traceId = options.traceId ?? randomUUID()
  const sourceSystem = options.sourceSystem ?? 'forge_observer'

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
  }
}
