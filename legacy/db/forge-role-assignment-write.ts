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

/**
 * The one decision the CONTRACT write makes about its three array columns.
 *
 * `forge_role_contract` carried the same `case when cardinality(excluded.x) > 0 then excluded.x else
 * <existing> end` rule for finding_ids, merge_checks and surface_scope — the last non-empty write won,
 * silently. That was a SECOND writer beside the assignment decider, and a shrunken surface_scope makes two
 * genuinely overlapping lanes read as disjoint. Each column is now decided by the SAME rule above, against
 * its OWN existing value: an add unions, an empty write is a no-op, and a proper subset is refused naming
 * the column and the entries it would drop. One rule, reused — not a second SQL case.
 */

export type ContractWriteSet = {
  findingIds: string[]
  mergeChecks: string[]
  surfaceScope: string[]
}

export type ContractWriteDecision =
  | { kind: 'allow'; findingIds: string[]; mergeChecks: string[]; surfaceScope: string[] }
  | { kind: 'refuse'; column: 'finding_ids' | 'merge_checks' | 'surface_scope'; dropped: string[] }

export function decideContractWrite(
  existing: ContractWriteSet | null,
  incoming: ContractWriteSet,
): ContractWriteDecision {
  const findingIds = decideAssignmentWrite(
    { assignmentId: 'finding_ids', attempt: 0, findingIds: existing?.findingIds ?? [] },
    { assignmentId: 'finding_ids', attempt: 0, findingIds: incoming.findingIds },
  )
  if (findingIds.kind === 'refuse') {
    return { kind: 'refuse', column: 'finding_ids', dropped: findingIds.dropped }
  }
  const mergeChecks = decideAssignmentWrite(
    { assignmentId: 'merge_checks', attempt: 0, findingIds: existing?.mergeChecks ?? [] },
    { assignmentId: 'merge_checks', attempt: 0, findingIds: incoming.mergeChecks },
  )
  if (mergeChecks.kind === 'refuse') {
    return { kind: 'refuse', column: 'merge_checks', dropped: mergeChecks.dropped }
  }
  const surfaceScope = decideAssignmentWrite(
    { assignmentId: 'surface_scope', attempt: 0, findingIds: existing?.surfaceScope ?? [] },
    { assignmentId: 'surface_scope', attempt: 0, findingIds: incoming.surfaceScope },
  )
  if (surfaceScope.kind === 'refuse') {
    return { kind: 'refuse', column: 'surface_scope', dropped: surfaceScope.dropped }
  }
  return {
    kind: 'allow',
    findingIds: findingIds.findingIds,
    mergeChecks: mergeChecks.findingIds,
    surfaceScope: surfaceScope.findingIds,
  }
}
