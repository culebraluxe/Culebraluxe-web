// ---------------------------------------------------------------------------
// Forge Alerts — separate from Observer.
//
// Observer appends facts. Alerts read a snapshot and emit recommendations.
// Nothing here writes traces, talks to Git, or advances an engine token.
// ---------------------------------------------------------------------------

import type { TraceEvent } from '../forge-observer/types'

export type AlertSeverity = 'info' | 'watch' | 'hold-recommend'

export type Alert = {
  code: string
  severity: AlertSeverity
  storyId: string
  nodeId?: string
  taskId?: string
  reason: string
  eventSeqs: number[]
}

export type AlertRule = {
  code: string
  evaluate(events: TraceEvent[]): Alert[]
}
