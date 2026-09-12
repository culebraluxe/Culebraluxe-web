// ---------------------------------------------------------------------------
// In-memory TraceSink for tests and observer-mode. Swap for Neon later.
// Fail-closed: append never invents identity fields.
// ---------------------------------------------------------------------------

import type { TraceEvent, TraceSink } from './types'

export type MemoryTraceSink = TraceSink & {
  snapshot(storyId?: string): TraceEvent[]
  /**
   * FORGE-OBS-LIST-01 — take a previously-persisted history into this process.
   *
   * `seq` is process-local by construction, and it is also part of the durable
   * de-dupe key (`storyId:taskId:nodeId:attempt:seq` in persistent-sink.ts). If a
   * restarted process restarted numbering at 1, a resumed attempt would build the
   * SAME source_event_id as the pre-restart write and the trace table's
   * `on conflict do nothing` would silently drop the new event — the observer
   * would lose exactly the events a restart makes interesting. So seeding also
   * advances the counter: new events always get numbers nobody has used.
   *
   * Events are appended in the order given; callers pass them oldest-first.
   */
  seed(events: TraceEvent[]): void
}

export function createMemoryTraceSink(): MemoryTraceSink {
  const events: TraceEvent[] = []
  let seq = 0

  return {
    append(partial) {
      if (!partial.storyId) throw new Error('TraceSink.append: missing storyId')
      if (!partial.taskId) throw new Error('TraceSink.append: missing taskId')
      if (!partial.nodeId) throw new Error('TraceSink.append: missing nodeId')
      if (!Number.isInteger(partial.attempt) || partial.attempt < 1) {
        throw new Error('TraceSink.append: attempt must be a 1-based integer')
      }
      seq += 1
      const event: TraceEvent = {
        ...partial,
        seq,
        atIso: partial.atIso ?? new Date().toISOString(),
        paths: partial.paths,
      }
      events.push(event)
      return event
    },
    list(storyId) {
      return events.filter((e) => e.storyId === storyId)
    },
    snapshot(storyId) {
      return storyId ? events.filter((e) => e.storyId === storyId) : [...events]
    },
    seed(seedEvents) {
      for (const event of seedEvents) {
        events.push(event)
        // The counter may only move forward: see the note above.
        if (Number.isInteger(event.seq) && event.seq > seq) seq = event.seq
      }
    },
  }
}
