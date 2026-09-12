// ---------------------------------------------------------------------------
// ENG-FORGE-OBS-SERIAL-01 — the runner's single observer seam.
//
// The observer package (forge-observer/) records; it does not know about lanes,
// assignments or routing. This module is the ONE place the runner talks to it,
// so the hook has one shape and can be exercised with a fake sink — no OpenCode,
// no database — instead of only through a live run.
//
// PHASE 1 DISCIPLINE (unchanged): instruments are not authority. Nothing here
// advances a token, and nothing here throws. A `hold-recommend` alert is
// RECORDED, never thrown — the runner still owns every HOLD, by its own throw
// (never-do #1 in the FORGE HOLES work order).
//
// The gap this closes: the runner recorded the worker-execution layer for SPLIT
// children only, so serial Smith / Lead / QA — the path that actually runs every
// story — was dark. No SCOPE_CHECK, no GIT_COMMIT, no HOLD was ever written for
// it, which made scorecard split health the only measured lane.
// ---------------------------------------------------------------------------

import { evaluateAlerts, type Alert } from './forge-alerts'
import { scopeViolations, type SmithExecutionContract } from './smith-contract'
import {
  recordAlert,
  recordGitCommit,
  recordHold,
  recordRunEnd,
  recordRunStart,
  recordScopeCheck,
  retryInputHash,
  type RecordBase,
  type TraceSink,
} from './forge-observer'

export type AttemptStatus = 'completed' | 'failed' | 'interrupted'

/**
 * One role attempt begins. `route` carries the Lead's chosen route on lead_pre
 * (SOLO/SMITH/SPLIT/HOLD) without inventing an event kind: the chosen route is
 * recorded on run.start, which is already in the union.
 */
export function observeAttemptBegin(
  sink: TraceSink,
  base: RecordBase,
  input: { role: string; route?: string | null },
): void {
  recordRunStart(sink, base, {
    role: input.role,
    ...(input.route ? { route: input.route } : {}),
  })
}

/** One role attempt ends, then the alert rules are drained over the story's trace. */
export function observeAttemptEnd(
  sink: TraceSink,
  base: RecordBase,
  input: { status: AttemptStatus; sha?: string; storyId: string },
): void {
  recordRunEnd(sink, base, { status: input.status, sha: input.sha })
  drainAlerts(sink, base, input.storyId)
}

/**
 * The candidate a worker produced: its commit is a FACT regardless of scope, so
 * it is always recorded; the SCOPE_CHECK is recorded only when a contract
 * exists to check against (a lane with no accepted assignment has no declared
 * scope to violate).
 *
 * Returns the violations so the CALLER decides whether they are a HOLD. This
 * function never throws and never gates — the split lane HOLDs on a violation,
 * the serial lane measures it (see the runner for why).
 */
export function observeCandidateCommit(
  sink: TraceSink,
  base: RecordBase,
  input: { candidateSha: string; changedFiles: string[]; contract: SmithExecutionContract | null },
): { violations: string[]; scopeChecked: boolean } {
  recordGitCommit(sink, base, {
    sha: input.candidateSha,
    paths: input.changedFiles,
    changed: input.changedFiles.length > 0,
  })
  if (!input.contract) return { violations: [], scopeChecked: false }
  const violations = scopeViolations(input.contract, input.changedFiles)
  recordScopeCheck(sink, base, {
    sha: input.candidateSha,
    paths: input.changedFiles,
    violations,
  })
  return { violations, scopeChecked: true }
}

/**
 * A HOLD the runner is about to throw. `missReasons` (when present) becomes the
 * retry hash, which is what makes RETRY_UNCHANGED_INPUT able to say "attempt 3
 * repeated attempt 2's input" instead of only counting attempts.
 */
export function observeHold(
  sink: TraceSink,
  base: RecordBase,
  input: { reasons: string[]; sha?: string; missReasons?: string[] | null },
): void {
  const misses = (input.missReasons ?? []).map((r) => r.trim()).filter(Boolean)
  recordHold(sink, base, {
    reasons: input.reasons,
    sha: input.sha,
    retryHash: misses.length > 0 ? retryInputHash({ missReasons: misses }) : null,
  })
}

/** The defined route was HOLD — a decision, not a runner failure. Recorded, not thrown. */
export function observeRouteHold(sink: TraceSink, base: RecordBase, reason: string): void {
  recordHold(sink, base, { reasons: [`route:HOLD ${reason}`.trim()] })
}

/**
 * A measurement that could not be taken.
 *
 * Recorded rather than swallowed: an observer that silently fails to observe is
 * how a lane goes dark again, and the whole point of this story is that the
 * serial lane stops being dark. Nothing depends on this event.
 */
export function observeMeasurementGap(
  sink: TraceSink,
  base: RecordBase,
  input: { what: string; reason: string },
): void {
  recordAlert(sink, base, {
    code: `UNMEASURED_${input.what}`,
    severity: 'info',
    reason: input.reason,
  })
}

/**
 * Evaluate the story's alert rules and record what is NEW.
 *
 * Deduplicated against the alert events already in this story's trace: the
 * runner drains alerts after several moments in one attempt, and the rules read
 * the whole story history, so an undeduplicated drain wrote the same alert again
 * on every drain and every retry. Codes/reasons are stable, so an alert is
 * recorded once and re-recorded only if its text actually changes.
 */
export function drainAlerts(sink: TraceSink, base: RecordBase, storyId: string): Alert[] {
  const events = sink.list(storyId)
  const already = new Set(
    events
      .filter((e) => e.kind === 'alert')
      .map((e) => `${String(e.detail?.code ?? '')}::${e.reason ?? ''}`),
  )
  const recorded: Alert[] = []
  for (const alert of evaluateAlerts(events)) {
    const key = `${alert.code}::${alert.code}: ${alert.reason}`
    if (already.has(key)) continue
    recordAlert(sink, base, {
      code: alert.code,
      severity: alert.severity,
      reason: alert.reason,
    })
    already.add(key)
    recorded.push(alert)
  }
  return recorded
}
