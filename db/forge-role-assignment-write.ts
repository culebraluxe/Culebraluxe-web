/**
 * The one decision a plan write makes about an assignment's findings.
 *
 * `scripts/forge-handoff.mjs` upserted `forge_role_assignment.finding_ids` with
 * `case when cardinality(excluded.finding_ids) > 0 then excluded.finding_ids else <existing> end`
 * — last non-empty write wins. Three chunk writes against ONE assignment therefore kept only the
 * last chunk's findings, and the 2026-09-17 hand-off left F1/F2/F5/F6 unassigned. The rule that
 * owns that write lives here, once, pure and DB-free, so the script and its test read the same
 * decision instead of two copies of a SQL case.
 *
 * ADD IS A UNION, REPLAY IS A NO-OP, SHRINK IS REFUSED.
 * - `dropped` empty (the write adds or repeats what is already held): allow, store the union.
 * - `incoming` empty: allow, store the existing set unchanged — a chunk that declares no finding
 *   binds nothing new and must not erase what earlier chunks bound.
 * - otherwise the write declares a non-empty proper subset of what the assignment already holds:
 *   a replace would drop those findings, so refuse and name them. A disjoint chunk both adds and
 *   drops under a replace, which is why the add test comes first — refusing it would break the
 *   multi-chunk plan this story exists to allow.
 */

export type AssignmentFindingSet = {
  assignmentId: string
  attempt: number
  findingIds: string[]
}

export type AssignmentWriteDecision =
  | { kind: 'allow'; findingIds: string[] }
  | { kind: 'refuse'; assignmentId: string; attempt: number; dropped: string[] }

function unique(ids: string[]): string[] {
  return [...new Set(ids)]
}

export function decideAssignmentWrite(
  existing: AssignmentFindingSet | null,
  incoming: AssignmentFindingSet,
): AssignmentWriteDecision {
  const held = existing ? unique(existing.findingIds) : []
  const declared = unique(incoming.findingIds)
  const dropped = held.filter((id) => !declared.includes(id))
  const added = declared.filter((id) => !held.includes(id))
  if (dropped.length === 0 || added.length > 0) {
    const union = [...held]
    for (const id of declared) if (!union.includes(id)) union.push(id)
    return { kind: 'allow', findingIds: union }
  }
  if (declared.length === 0) {
    return { kind: 'allow', findingIds: held }
  }
  return {
    kind: 'refuse',
    assignmentId: incoming.assignmentId,
    attempt: incoming.attempt,
    dropped,
  }
}
