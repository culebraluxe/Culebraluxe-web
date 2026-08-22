// ---------------------------------------------------------------------------
// Closing-date timer reconciliation (Story 122 / 137) — thin wrapper over the
// generic CRM-22 deadline-timer seam (workflow_app/deadline-timer.ts).
//
// The RE_supermodel's `closing_date_timer` node (in the XML) owns scheduling
// the closing-deadline timer from the canonical `closingDate` fact. When that
// canonical date changes while the instance is active, this seam RESCHEDULES
// the instance's existing pending timer — the SAME workflow instance
// continues, never terminated/restarted. No legal deadline is invented: if no
// canonical closing date exists (or no timer has been scheduled yet), nothing
// happens; the node schedules it when the workflow reaches the closing stage.
//
// This module is generic: it names no workflow node and no jurisdiction.
// ---------------------------------------------------------------------------

import {
  reconcileDeadlineTimerCore,
  type DeadlineTimerDeps,
  type DeadlineTimerResult,
} from './deadline-timer'

export type ClosingTimerDeps = {
  findPendingTimer: (
    instanceId: string,
  ) => Promise<{ jobId: string; dueAt: string } | null>
  reschedule: (jobId: string, dueAt: Date) => Promise<void>
}

export type ClosingTimerResult = DeadlineTimerResult

const CLOSING_TIMER_NODE_ID = 'closing_date_timer'

export async function reconcileClosingTimerCore(
  instanceId: string,
  closingDate: string | null,
  deps: ClosingTimerDeps,
): Promise<ClosingTimerResult> {
  // The closing-date monitor is the instance's only timer in the legacy
  // single-timer world; the generic seam scopes the lookup by node id via the
  // job payload. The injected deps predate CRM-22 and look up without a node
  // id, which is exactly correct for a single-timer instance.
  return reconcileDeadlineTimerCore(instanceId, CLOSING_TIMER_NODE_ID, closingDate, {
    findPendingTimer: async (id) => deps.findPendingTimer(id),
    reschedule: deps.reschedule,
  } satisfies DeadlineTimerDeps)
}

export async function reconcileClosingTimer(
  instanceId: string,
  closingDate: string | null,
): Promise<ClosingTimerResult> {
  const { reconcileDeadlineTimer } = await import('./deadline-timer')
  return reconcileDeadlineTimer(instanceId, CLOSING_TIMER_NODE_ID, closingDate)
}
