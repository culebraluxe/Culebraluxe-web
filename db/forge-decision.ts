// ---------------------------------------------------------------------------
// FORGE DECISION repository (migration 180).
//
// Every write in this file goes through `decisionWritePolicy` BEFORE it touches SQL, so the role rules
// are enforced once, at the boundary, instead of being restated (and eventually forgotten) at each call
// site. A refusal throws `DecisionError` with a code and the policy's own reason, which is what the
// caller needs to explain itself: "Smith must not promote: a rule promoted by its implementer is not a
// rule" is a sentence, not a 500.
//
// The read used by the injector is one query: active decisions for a domain, newest promoted first,
// capped. It is deliberately small enough to run before every Lead/Smith lane without a cache.
// ---------------------------------------------------------------------------

import {
  DECISION_INJECTION_CAP,
  decisionWritePolicy,
  isValidDecisionKey,
  validateStatement,
  type ForgeDecision,
  type ForgeDecisionRole,
  type ForgeDecisionSource,
  type ForgeDecisionStatus,
  type ForgeDecisionDomain,
  type ForgeDecisionAction,
} from '../lib/forge-decision'
import type { QueryExecutor } from './query-executor'

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export class DecisionError extends Error {
  code: 'policy-denied' | 'invalid' | 'not-found'
  constructor(code: DecisionError['code'], message: string) {
    super(message)
    this.name = 'DecisionError'
    this.code = code
  }
}

export function isoOrNull(value: unknown): string | null {
  if (value == null) return null
  if (value instanceof Date) return value.toISOString()
  const parsed = Date.parse(String(value))
  return Number.isNaN(parsed) ? null : new Date(parsed).toISOString()
}

export function mapDecision(row: Record<string, unknown>): ForgeDecision {
  return {
    id: String(row.id),
    key: String(row.key),
    statement: String(row.statement),
    status: String(row.status) as ForgeDecisionStatus,
    owner: row.owner == null ? null : String(row.owner),
    evidenceSha: row.evidence_sha == null ? null : String(row.evidence_sha),
    supersedesId: row.supersedes_id == null ? null : String(row.supersedes_id),
    source: String(row.source) as ForgeDecisionSource,
    domain: String(row.domain) as ForgeDecisionDomain,
    createdAt: isoOrNull(row.created_at) ?? '',
    promotedAt: isoOrNull(row.promoted_at),
    supersededAt: isoOrNull(row.superseded_at),
  }
}

/** The enforcement point: refuse in code, with the policy's reason, before writing. */
export function assertDecisionWrite(role: ForgeDecisionRole, action: ForgeDecisionAction): void {
  const verdict = decisionWritePolicy(role, action)
  if (!verdict.allowed) {
    throw new DecisionError('policy-denied', verdict.reason)
  }
}

/**
 * The column list is written LITERALLY in each query below, not held in a constant: `QueryExecutor` is a
 * tagged template, so an interpolated `${COLUMNS}` would arrive as a single bound parameter rather than
 * a column list. `db/agent-work.ts` carries the same note for the same reason.
 */

/**
 * ACTIVE DECISIONS FOR A DOMAIN — the injector's query.
 *
 * Newest promoted first, capped at the packet's limit. The order is `promoted_at desc` and not
 * `created_at`: the decision promoted most recently is the correction, so it should read first.
 */
export async function listActiveDecisions(
  domain: ForgeDecisionDomain,
  options: { limit?: number } = {},
  execute?: QueryExecutor,
): Promise<ForgeDecision[]> {
  const q = execute ?? (await executor())
  const cap = Math.min(DECISION_INJECTION_CAP, Math.max(1, Math.trunc(options.limit ?? DECISION_INJECTION_CAP)))
  const rows = (await q`
    select id, key, statement, status, owner, evidence_sha, supersedes_id, source, domain,
      created_at, promoted_at, superseded_at
    from forge_decision
    where status = 'active' and domain = ${domain}
    order by promoted_at desc nulls last, key
    limit ${cap}
  `) as Record<string, unknown>[]
  return rows.map(mapDecision)
}

/** Every decision, for the mirror and the CLI. Newest first, no cap. */
export async function listDecisions(
  options: { status?: ForgeDecisionStatus } = {},
  execute?: QueryExecutor,
): Promise<ForgeDecision[]> {
  const q = execute ?? (await executor())
  const rows = options.status
    ? ((await q`
        select id, key, statement, status, owner, evidence_sha, supersedes_id, source, domain,
          created_at, promoted_at, superseded_at
        from forge_decision
        where status = ${options.status}
        order by created_at desc
      `) as Record<string, unknown>[])
    : ((await q`
        select id, key, statement, status, owner, evidence_sha, supersedes_id, source, domain,
          created_at, promoted_at, superseded_at
        from forge_decision
        order by created_at desc
      `) as Record<string, unknown>[])
  return rows.map(mapDecision)
}

export async function getDecision(key: string, execute?: QueryExecutor): Promise<ForgeDecision | null> {
  const q = execute ?? (await executor())
  const rows = (await q`
    select id, key, statement, status, owner, evidence_sha, supersedes_id, source, domain,
      created_at, promoted_at, superseded_at
    from forge_decision
    where key = ${key}
    limit 1
  `) as Record<string, unknown>[]
  return rows[0] ? mapDecision(rows[0]) : null
}

export type InsertDecisionInput = {
  key: string
  statement: string
  domain?: ForgeDecisionDomain
  source?: ForgeDecisionSource
  evidenceSha?: string | null
  /** The decision this one replaces, by key. Resolved here so callers never handle ids. */
  supersedesKey?: string | null
}

/**
 * INSERT A CANDIDATE.
 *
 * The status is not a parameter: this function only ever writes `candidate`, which is what makes "Scout
 * may insert candidates only" impossible to violate by passing `status: 'active'`. Promotion is a
 * separate, separately-guarded function.
 */
export async function insertDecisionCandidate(
  input: InsertDecisionInput,
  actor: { role: ForgeDecisionRole; name?: string | null },
  execute?: QueryExecutor,
): Promise<ForgeDecision> {
  assertDecisionWrite(actor.role, 'insert-candidate')

  const problems = validateStatement(input.statement)
  if (!isValidDecisionKey(input.key)) {
    problems.push(`key "${input.key}" must be a lowercase slug (a-z, 0-9, dashes)`)
  }
  if (problems.length > 0) throw new DecisionError('invalid', problems.join('; '))

  const q = execute ?? (await executor())
  const supersedesId = input.supersedesKey ? ((await getDecision(input.supersedesKey, q))?.id ?? null) : null
  const rows = (await q`
    insert into forge_decision (key, statement, status, source, domain, evidence_sha, supersedes_id)
    values (
      ${input.key}, ${input.statement.trim()}, 'candidate',
      ${input.source ?? 'packet'}, ${input.domain ?? 'forge'}, ${input.evidenceSha ?? null}, ${supersedesId}
    )
    returning id, key, statement, status, owner, evidence_sha, supersedes_id, source, domain,
      created_at, promoted_at, superseded_at
  `) as Record<string, unknown>[]
  return mapDecision(rows[0])
}

/**
 * PROMOTE — candidate to active. Architect or the captain only.
 *
 * `owner` is required by the database's promotion-shape check, and it is required here too so the failure
 * is a clear sentence rather than a constraint name. Promoting an already-active decision is refused
 * rather than treated as success: the cap on re-promotion is what stops `promoted_at` from being a
 * "bump me to the top of the injection list" button.
 */
export async function promoteDecision(
  key: string,
  actor: { role: ForgeDecisionRole; name?: string | null },
  options: { evidenceSha?: string | null } = {},
  execute?: QueryExecutor,
): Promise<ForgeDecision> {
  assertDecisionWrite(actor.role, 'promote')
  const q = execute ?? (await executor())

  const existing = await getDecision(key, q)
  if (!existing) throw new DecisionError('not-found', `no decision with key "${key}"`)
  if (existing.status === 'active') {
    throw new DecisionError('invalid', `decision "${key}" is already active (promoted ${existing.promotedAt})`)
  }
  if (existing.status === 'superseded') {
    throw new DecisionError('invalid', `decision "${key}" is superseded; write a new candidate instead`)
  }

  const owner = actor.name?.trim() || actor.role
  const rows = (await q`
    update forge_decision
    set status = 'active', owner = ${owner}, promoted_at = now(),
        evidence_sha = coalesce(${options.evidenceSha ?? null}, evidence_sha)
    where key = ${key} and status = 'candidate'
    returning id, key, statement, status, owner, evidence_sha, supersedes_id, source, domain,
      created_at, promoted_at, superseded_at
  `) as Record<string, unknown>[]
  if (!rows[0]) throw new DecisionError('not-found', `decision "${key}" changed while being promoted`)
  return mapDecision(rows[0])
}

/**
 * SUPERSEDE — an active decision stops being in force, without being deleted.
 *
 * Deleting would erase the fact that the rule existed and was wrong, which is the one thing the audit
 * trail is for. The replacement is a separate candidate promoted on its own, so the pair (`supersedes_id`)
 * reads as a history rather than a swap.
 */
export async function supersedeDecision(
  key: string,
  actor: { role: ForgeDecisionRole; name?: string | null },
  execute?: QueryExecutor,
): Promise<ForgeDecision> {
  assertDecisionWrite(actor.role, 'supersede')
  const q = execute ?? (await executor())
  const existing = await getDecision(key, q)
  if (!existing) throw new DecisionError('not-found', `no decision with key "${key}"`)
  if (existing.status !== 'active') {
    throw new DecisionError('invalid', `decision "${key}" is ${existing.status}; only an active one can be superseded`)
  }
  const rows = (await q`
    update forge_decision
    set status = 'superseded', superseded_at = now()
    where key = ${key} and status = 'active'
    returning id, key, statement, status, owner, evidence_sha, supersedes_id, source, domain,
      created_at, promoted_at, superseded_at
  `) as Record<string, unknown>[]
  if (!rows[0]) throw new DecisionError('not-found', `decision "${key}" changed while being superseded`)
  return mapDecision(rows[0])
}

