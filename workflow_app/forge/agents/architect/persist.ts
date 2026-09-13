import { renderArchitectHandoff } from '../architect-handoff'
import type { ArchitectHandoff } from '../architect-handoff'

const BRIEF_PROSE_CAP = 4000

/**
 * What to write on the story. The handoff line is NEVER sliced.
 * Prose is optional commentary and may be capped.
 */
export function persistArchitectBrief(prose: string, handoff: ArchitectHandoff): string {
  const clean = prose.replace(/FORGE_ARCHITECT_HANDOFF:\s*\{[\s\S]*$/, '').trim()
  const clipped =
    clean.length > BRIEF_PROSE_CAP
      ? `${clean.slice(0, BRIEF_PROSE_CAP)}\n[Architect prose truncated; contract is the handoff line]`
      : clean
  return [clipped, renderArchitectHandoff(handoff)].filter(Boolean).join('\n\n')
}

/**
 * The work orders a Smith lane is actually handed: the findings the Lead claimed,
 * read straight off the Architect handoff so the words are the Architect's, not a
 * paraphrase. Read-only — this builds text, it does not route.
 */
export function smithWorkOrdersFromFindings(
  findingIds: string[],
  handoff: ArchitectHandoff,
): string {
  const lines = [
    'WORK ORDERS from Architect findings (execute in assignment order; do NOT enlarge scope):',
  ]
  for (const id of findingIds) {
    const f = handoff.findings.find((row) => row.id === id)
    if (!f) continue
    lines.push(`Finding ${f.id}: ${f.summary}`)
    lines.push(`  Preconditions: ${f.preconditions.join('; ') || '—'}`)
    lines.push(`  Scope: ${f.scope.join(', ')}`)
    lines.push(`  Postconditions: ${f.postconditions.join('; ') || '—'}`)
    lines.push(`  Classes: ${f.classes.join(', ') || '—'}`)
    lines.push(`  Risks: ${f.risks.join('; ') || '—'}`)
  }
  return lines.join('\n')
}
