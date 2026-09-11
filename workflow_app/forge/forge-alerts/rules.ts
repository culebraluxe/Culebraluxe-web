import type { TraceEvent } from '../forge-observer/types'
import type { Alert, AlertRule } from './types'

function storyOf(events: TraceEvent[]): string {
  return events[0]?.storyId ?? ''
}

/** Scope deny at candidate capture — lock is working. */
export const scopeDeniedRule: AlertRule = {
  code: 'SCOPE_DENIED',
  evaluate(events) {
    return events
      .filter((e) => e.kind === 'scope.check' && e.verdict === 'deny')
      .map((e) => ({
        code: 'SCOPE_DENIED',
        severity: 'hold-recommend' as const,
        storyId: e.storyId,
        nodeId: e.nodeId,
        taskId: e.taskId,
        reason: e.reason ?? 'candidate paths outside allowedScope',
        eventSeqs: [e.seq],
      }))
  },
}

/**
 * Two siblings in the same process wrote the same file.
 * Join may still pass if both produced SHAs — this names the collision.
 */
export const siblingCollisionRule: AlertRule = {
  code: 'SIBLING_FILE_COLLISION',
  evaluate(events) {
    const writes = events.filter(
      (e) =>
        (e.kind === 'git.commit' || e.kind === 'scope.check') &&
        e.verdict === 'allow' &&
        (e.paths?.length ?? 0) > 0,
    )
    const byFile = new Map<string, TraceEvent[]>()
    for (const e of writes) {
      for (const p of e.paths ?? []) {
        const list = byFile.get(p) ?? []
        list.push(e)
        byFile.set(p, list)
      }
    }
    const alerts: Alert[] = []
    for (const [path, hits] of byFile) {
      const owners = new Set(hits.map((h) => h.nodeId + ':' + (h.detail?.assignmentId ?? h.taskId)))
      if (owners.size < 2) continue
      alerts.push({
        code: 'SIBLING_FILE_COLLISION',
        severity: 'hold-recommend',
        storyId: storyOf(hits),
        reason: `file ${path} written by ${owners.size} siblings`,
        eventSeqs: hits.map((h) => h.seq),
      })
    }
    return alerts
  },
}

/**
 * Self-heal / second attempt with the same miss+packet+assignment hash.
 * Shepherd "replay from the change" — we refuse a no-op retry.
 */
export const unchangedRetryRule: AlertRule = {
  code: 'RETRY_UNCHANGED_INPUT',
  evaluate(events) {
    const holds = events.filter((e) => e.kind === 'hold' && e.detail?.retryHash)
    const seen = new Map<string, TraceEvent>()
    const alerts: Alert[] = []
    for (const e of holds) {
      const hash = String(e.detail?.retryHash)
      const prior = seen.get(hash)
      if (prior) {
        alerts.push({
          code: 'RETRY_UNCHANGED_INPUT',
          severity: 'hold-recommend',
          storyId: e.storyId,
          nodeId: e.nodeId,
          taskId: e.taskId,
          reason: `attempt ${e.attempt} repeated retry hash ${hash} from attempt ${prior.attempt}`,
          eventSeqs: [prior.seq, e.seq],
        })
      } else {
        seen.set(hash, e)
      }
    }
    return alerts
  },
}

/** Publish path with no deploy receipt — parked gap, now visible. */
export const missingDeployReceiptRule: AlertRule = {
  code: 'MISSING_DEPLOY_RECEIPT',
  evaluate(events) {
    return events
      .filter((e) => e.kind === 'run.end' && e.nodeId === 'dev_ops' && e.detail?.hasReleaseEvidence === false)
      .map((e) => ({
        code: 'MISSING_DEPLOY_RECEIPT',
        severity: 'watch' as const,
        storyId: e.storyId,
        nodeId: e.nodeId,
        taskId: e.taskId,
        reason: 'dev_ops completed without releaseEvidence',
        eventSeqs: [e.seq],
      }))
  },
}

/** QA refused a candidate that already passed scope — implementation, not orchestration. */
export const qaAfterCleanScopeRule: AlertRule = {
  code: 'QA_AFTER_CLEAN_SCOPE',
  evaluate(events) {
    const alerts: Alert[] = []
    const bySha = new Map<string, TraceEvent[]>()
    for (const e of events) {
      if (!e.sha) continue
      const list = bySha.get(e.sha) ?? []
      list.push(e)
      bySha.set(e.sha, list)
    }
    for (const [sha, list] of bySha) {
      const clean = list.some((e) => e.kind === 'scope.check' && e.verdict === 'allow')
      const qaHold = list.find(
        (e) => e.kind === 'hold' && e.nodeId === 'qa' && /CODE_DEFECT|qaPassed:false/i.test(e.reason ?? ''),
      )
      if (clean && qaHold) {
        alerts.push({
          code: 'QA_AFTER_CLEAN_SCOPE',
          severity: 'info',
          storyId: qaHold.storyId,
          nodeId: 'qa',
          reason: `SHA ${sha.slice(0, 12)} was in-lane; QA refused on product grounds`,
          eventSeqs: list.map((e) => e.seq),
        })
      }
    }
    return alerts
  },
}

export function defaultAlertRules(): AlertRule[] {
  return [
    scopeDeniedRule,
    siblingCollisionRule,
    unchangedRetryRule,
    missingDeployReceiptRule,
    qaAfterCleanScopeRule,
  ]
}
