import type { TraceEvent } from '../forge-observer/types'
import type { Alert, AlertRule } from './types'
import { defaultAlertRules } from './rules'

export function evaluateAlerts(
  events: TraceEvent[],
  rules: AlertRule[] = defaultAlertRules(),
): Alert[] {
  const out: Alert[] = []
  for (const rule of rules) {
    out.push(...rule.evaluate(events))
  }
  return out.sort((a, b) => severityRank(a.severity) - severityRank(b.severity) || a.code.localeCompare(b.code))
}

function severityRank(s: Alert['severity']): number {
  if (s === 'hold-recommend') return 0
  if (s === 'watch') return 1
  return 2
}

export function holdRecommendations(alerts: Alert[]): Alert[] {
  return alerts.filter((a) => a.severity === 'hold-recommend')
}
