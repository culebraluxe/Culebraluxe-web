import { pathAllowed } from '../shared/path'
import type { SmithAssignment } from './types'
import { serialScopeMissReasons } from '../../forge-serial-doors'

export function scopeViolations(changedPaths: string[], assignment: SmithAssignment): string[] {
  return changedPaths.filter((p) => !pathAllowed(p, assignment.allowedScope, assignment.prohibitedScope))
}

export function assessSmithScope(input: {
  assignment: SmithAssignment
  changedPaths: string[]
}): { ok: true } | { ok: false; reasons: string[] } {
  const hits = scopeViolations(input.changedPaths, input.assignment)
  if (!hits.length) return { ok: true }
  return { ok: false, reasons: serialScopeMissReasons(hits, input.assignment.id) }
}
