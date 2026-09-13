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
    'End with exactly one un-fenced JSON line SMITH_CANDIDATE: {"version":1,"assignmentId":"' +
      assignment.id +
      '","candidateSha":"<HEAD sha>","mergeBase":"<given base>","changedPaths":["..."]}.',
    'The runner will re-diff HEAD against merge base and refuse any path outside allowed scope. Your changedPaths list is a claim; git is the authority.',
  ].join('\n')
}
