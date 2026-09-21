import { fileOf, unique } from '@/legacy/workflow_app/forge/agents/shared/path'
import type { ArchitectHandoff } from '@/legacy/workflow_app/forge/agents/architect-handoff'
// The seam ceiling has ONE definition and the live architect contract owns it.
import { MAX_SEAMS_PER_FINDING, seamForNewFile } from '@/legacy/workflow_app/forge/forge-shaping'

/** Return true if `repoPath` exists as a blob or tree on `baseRef`. */
export type SeamExists = (baseRef: string, repoPath: string) => boolean

export type ArchitectAssessment =
  | { ok: true; handoff: ArchitectHandoff; advisories: string[] }
  | { ok: false; reasons: string[]; handoff: ArchitectHandoff | null }

export const ARCHITECT_HANDOFF_MISSING =
  'ARCHITECT: no readable handoff. End the reply with exactly ONE un-fenced JSON line beginning FORGE_ARCHITECT_HANDOFF: {version:1,baseRef:"<sha>",findings:[{id,required,summary,preconditions,scope,postconditions,classes,risks,hint}]}.'

/**
 * The Architect's own assessment of its handoff. Fail-closed: an invented seam
 * (a file that does not exist on the pinned baseRef) is a reason to HOLD, not a
 * plan to route. Reasons are written for the model to act on — this text feeds
 * the bounded self-heal reprompt, so a generic message wastes the retry.
 */
export function assessArchitectHandoff(
  handoff: ArchitectHandoff | null,
  options: { existsOnBaseRef?: SeamExists; maxSeams?: number } = {},
): ArchitectAssessment {
  const reasons: string[] = []
  const advisories: string[] = []
  if (!handoff) return { ok: false, reasons: [ARCHITECT_HANDOFF_MISSING], handoff: null }

  const maxSeams = options.maxSeams ?? MAX_SEAMS_PER_FINDING
  if (!unique(handoff.findings.map((f) => f.id))) reasons.push('Duplicate finding ids')

  const required = handoff.findings.filter((f) => f.required)
  if (required.length === 0) {
    reasons.push('No required findings. Adjacent-only discovery is a HOLD or a new story, not a silent empty plan.')
  }

  for (const f of handoff.findings) {
    const prefix = `${f.id}: `
    if (f.scope.length === 0) reasons.push(`${prefix}scope is empty`)
    if (f.scope.length > maxSeams) {
      reasons.push(`${prefix}scope has ${f.scope.length} seams; recut (max ${maxSeams})`)
    }
    for (const seam of f.scope) {
      if (!fileOf(seam)) reasons.push(`${prefix}illegal scope path ${seam}`)
    }
    if (f.required && f.hint === 'HOLD' && f.risks.length === 0) {
      reasons.push(`${prefix}required HOLD must name a concrete risk`)
    }
    if (!f.required && f.hint === 'SAME_UNIT') {
      advisories.push(`${prefix}adjacent finding marked SAME_UNIT — Lead will ignore it as current-story work`)
    }
    if (options.existsOnBaseRef && handoff.baseRef) {
      for (const seam of f.scope) {
        const path = fileOf(seam)
        if (!path) continue
        if (!options.existsOnBaseRef(handoff.baseRef, path)) {
          // A NEW FILE IS NOT ITS OWN SEAM, AND A SIBLING FILE IS NOT ITS SURFACE. Name the FIX, not just the
          // fault: these reasons feed the bounded self-heal reprompt, and a generic message wastes the retry.
          // The Architect that declared a sibling file on 2026-09-15 spent a whole HOLD learning what this
          // sentence says — the seam for a new file is its directory, and `seamForNewFile` owns that rule.
          reasons.push(
            `${prefix}scope ${path} does not exist on ${handoff.baseRef.slice(0, 12)} — if this file is NEW, ` +
              `declare its directory instead: ${seamForNewFile(path)}`,
          )
        }
      }
    }
  }

  if (required.length > 0 && !handoff.baseRef && options.existsOnBaseRef) {
    reasons.push('baseRef is required when existence is enforced')
  } else if (!handoff.baseRef) {
    advisories.push('baseRef empty — existence check skipped')
  }

  if (reasons.length) return { ok: false, reasons, handoff }
  return { ok: true, handoff, advisories }
}
