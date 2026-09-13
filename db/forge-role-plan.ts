/**
 * The work-order plan, in ROWS (migration 171).
 *
 * Replaces parsing a `LEAD_PLAN:` JSON line. A chunk row carries surface and proof as
 * NOT NULL columns, so "a chunk that cannot be checked" never reaches this reader.
 * Returns null when no rows were written OR when an assignment is incomplete — the
 * caller then falls back to whatever the reply said, rather than inventing a plan.
 */
import type { QueryExecutor } from './query-executor'

export type PlanChunk = {
  id: number
  outcome: string
  surface: string[]
  invariant: string
  proof: string
  dependsOn: number[]
}

export type PlanAssignment = {
  id: string
  findingIds: string[]
  dependsOn: string[]
  evidenceRefs: string[]
  reasoning: string
  features: Record<string, number>
  plan: { size: string; chunks: PlanChunk[] }
}

export type ForgeRolePlan = {
  size: 'SMALL' | 'MEDIUM' | 'LARGE'
  assignments: PlanAssignment[]
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/** The eight dispatchability numbers, or null when any of them is missing. */
function featuresOf(row: Record<string, unknown>): Record<string, number> | null {
  const map: Record<string, number> = {
    semanticSurface: Number(row.semantic_surface ?? NaN),
    dependencyDepth: Number(row.dependency_depth ?? NaN),
    uncertainty: Number(row.uncertainty ?? NaN),
    contextBurden: Number(row.context_burden ?? NaN),
    proofBurden: Number(row.proof_burden ?? NaN),
    coupling: Number(row.coupling ?? NaN),
    changeNovelty: Number(row.change_novelty ?? NaN),
    workerFit: Number(row.worker_fit ?? NaN),
  }
  for (const [key, value] of Object.entries(map)) {
    if (!Number.isInteger(value) || value <= 0) return null
    if (key === 'semanticSurface' || key === 'dependencyDepth') {
      if (value > 100) return null
    } else if (value > 5) {
      return null
    }
  }
  return map
}

export async function getForgeRolePlan(
  key: { taskId: string; nodeId: string; attempt: number },
  execute?: QueryExecutor,
): Promise<ForgeRolePlan | null> {
  const q = execute ?? (await executor())
  const assignments = await q`
    select assignment_id, finding_ids, evidence_refs, reasoning,
           semantic_surface, dependency_depth, uncertainty, context_burden,
           proof_burden, coupling, change_novelty, worker_fit
    from forge_role_assignment
    where task_id = ${key.taskId} and node_id = ${key.nodeId} and attempt = ${key.attempt}
    order by assignment_id
  `
  if (assignments.length === 0) return null

  const chunks = await q`
    select assignment_id, chunk_id, size, surface, proof, invariant,
           postconditions, depends_on
    from forge_role_plan_chunk
    where task_id = ${key.taskId} and node_id = ${key.nodeId} and attempt = ${key.attempt}
    order by assignment_id, chunk_id
  `

  const out: PlanAssignment[] = []
  let size: 'SMALL' | 'MEDIUM' | 'LARGE' = 'SMALL'
  for (const a of assignments) {
    const row = a as unknown as Record<string, unknown>
    const id = String(row.assignment_id)
    const features = featuresOf(row)
    if (!features) return null // incomplete vector: do not guess numbers the gate multiplies
    const own = chunks.filter((c) => (c as unknown as Record<string, unknown>).assignment_id === id)
    if (own.length === 0) return null
    out.push({
      id,
      findingIds: Array.isArray(row.finding_ids) ? (row.finding_ids as string[]) : [],
      dependsOn: [],
      evidenceRefs: Array.isArray(row.evidence_refs) ? (row.evidence_refs as string[]) : [],
      reasoning: String(row.reasoning ?? '(not stated)'),
      features,
      plan: {
        size: 'SMALL',
        chunks: own.map((c) => {
          const ch = c as unknown as Record<string, unknown>
          const post = Array.isArray(ch.postconditions) ? (ch.postconditions as string[]) : []
          const postText = post.join('; ')
          const chunkSize = (ch.size as string | null) ?? 'SMALL'
          if (chunkSize === 'MEDIUM') size = 'MEDIUM'
          return {
            id: Number(ch.chunk_id),
            // The reviewer wants an outcome and an invariant; when the model stated
            // only postconditions, those ARE the outcome and the invariant. Never
            // invent one — say it was not stated.
            outcome: postText || String(ch.invariant ?? '') || '(not stated)',
            surface: Array.isArray(ch.surface) ? (ch.surface as string[]) : [],
            invariant: String(ch.invariant ?? '') || postText || '(not stated)',
            proof: String(ch.proof ?? ''),
            dependsOn: Array.isArray(ch.depends_on)
              ? (ch.depends_on as string[]).map((d) => Number(d)).filter((n) => Number.isInteger(n))
              : [],
          }
        }),
      },
    })
  }
  for (const a of out) a.plan.size = size
  return { size, assignments: out }
}
