import type { SmithAssignment } from './types'

export function renderSmithWorkOrders(assignment: SmithAssignment): string {
  const lines = [
    `WORK ORDERS assignment ${assignment.id} (execute chunks in order; do NOT re-plan; do NOT enlarge scope):`,
    `Allowed scope: ${assignment.allowedScope.join(', ')}`,
    assignment.prohibitedScope.length
      ? `Prohibited: ${assignment.prohibitedScope.join(', ')}`
      : '',
  ]
  for (const chunk of assignment.chunks) {
    lines.push(`Chunk ${chunk.id}:`)
    lines.push(`  Preconditions: ${chunk.preconditions.join('; ') || '—'}`)
    lines.push(`  Scope: ${chunk.scope.join(', ')}`)
    lines.push(`  Postconditions: ${chunk.postconditions.join('; ') || '—'}`)
    lines.push(`  Classes: ${chunk.classes.join(', ') || '—'}`)
    lines.push(`  Risks: ${chunk.risks.join('; ') || '—'}`)
    lines.push(`  Proof you do not run (Assay runs this later): ${chunk.proof}`)
  }
  return lines.filter(Boolean).join('\n')
}

export function buildSmithDirective(assignment: SmithAssignment): string {
  return [
    'SMITH: implement the work orders. Do not choose scope. Do not route. Do not invent proofs.',
    renderSmithWorkOrders(assignment),
    'Commit on this worktree. Do not push.',
    'Do not emit SMITH_CANDIDATE or SMITH_PLAN JSON. The runner diffs HEAD against merge base and refuses any path outside allowed scope. Git is the cabinet.',
  ].join('\n')
}
