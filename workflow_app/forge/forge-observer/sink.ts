// ---------------------------------------------------------------------------
// In-memory TraceSink for tests and observer-mode. Swap for Neon later.
// Fail-closed: append never invents identity fields.
// ---------------------------------------------------------------------------

import type { TraceEvent, TraceSink } from './types'

export function createMemoryTraceSink(): TraceSink & { snapshot(storyId?: string): TraceEvent[] } {
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
  }
}
