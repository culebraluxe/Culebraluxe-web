// ---------------------------------------------------------------------------
// ENG-FORGE-HARDEN-01 — Durable Forge Knowledge Classes.
//
// Canonical, DB-free contract that makes Forge's durable information explicitly
// one of four classes so context consumers never treat discovered truth,
// architectural decisions, current execution state, and historical learning as
// interchangeable:
//
//   FACT       stable discovered truth (repo, env, architecture, interfaces)
//   DECISION   an architectural/execution decision made for current work
//   STATE      mutable authoritative runtime state (lane, owner, attempt,
//              candidate SHA, blocker, promotion, completion)
//   LESSON     durable knowledge from prior execution/failure evidence
//
// PRINCIPLES (no competing state authority):
//   - STATE comes only from the authoritative runtime; nothing here stores STATE.
//   - Current STATE outranks stale historical observations.
//   - Current story DECISIONS outrank historical LESSONS.
//   - LESSONS are context, never authority.
//   - Smith does not treat LESSONS as implementation authority.
//
// This module is pure and DB-free; it classifies and selects context. Persisting
// records delegates to existing Forge mechanisms (the caller supplies durable
// records); it never introduces a second state system.
// ---------------------------------------------------------------------------

export type ForgeKnowledgeClass = 'FACT' | 'DECISION' | 'STATE' | 'LESSON'

export type ForgeRole =
  | 'scout'
  | 'architect'
  | 'lead'
  | 'smith'
  | 'qa'
  | 'dev_ops'
  | 'lead_post'

export type KnowledgeScope =
  | 'repo' // stable, not story-scoped
  | 'story' // applies to one story
  | 'run' // one run/attempt

export type ForgeKnowledgeProvenance = {
  kind: 'engine' | 'architect' | 'scout' | 'lead' | 'run' | 'external' | 'operator'
  ref: string // durable identity (task/run/instance/decision id)
  atIso: string
}

export type ForgeKnowledgeRecord = {
  id: string
  class: ForgeKnowledgeClass
  scope: KnowledgeScope
  /** storyId when scope === 'story'; null for repo-global. */
  storyId: string | null
  title: string
  body: string
  /** When true this historical observation is stale vs current STATE. */
  stale?: boolean
  /** Classification that produced a LESSON (HARDEN-04 wiring). */
  sourceClass?: string
  provenance: ForgeKnowledgeProvenance
}

// ---------------------------------------------------------------------------
// Which classes each role may consume (role-specific context selection).
// STATE is authoritative for every role; LESSON is context-only and only
// surfaced where it informs future judgment (never Smith implementation).
// ---------------------------------------------------------------------------

const ROLE_CLASS_POLICY: Record<ForgeRole, ReadonlySet<ForgeKnowledgeClass>> = {
  scout: new Set(['FACT', 'STATE', 'LESSON']),
  architect: new Set(['FACT', 'DECISION', 'STATE', 'LESSON']),
  lead: new Set(['FACT', 'DECISION', 'STATE']),
  smith: new Set(['FACT', 'DECISION', 'STATE']), // NO LESSON authority
  qa: new Set(['FACT', 'DECISION', 'STATE']),
  dev_ops: new Set(['FACT', 'DECISION', 'STATE']),
  lead_post: new Set(['FACT', 'DECISION', 'STATE']),
}

/** Classify an observation into a canonical knowledge record. */
export function classifyKnowledge(input: {
  id: string
  class: ForgeKnowledgeClass
  scope: KnowledgeScope
  storyId?: string | null
  title: string
  body: string
  provenance: ForgeKnowledgeProvenance
}): ForgeKnowledgeRecord {
  return {
    id: input.id,
    class: input.class,
    scope: input.scope,
    storyId: input.storyId ?? null,
    title: input.title,
    body: input.body,
    provenance: input.provenance,
  }
}

export type ForgeContextSelection = {
  role: ForgeRole
  included: ForgeKnowledgeRecord[]
  /** Current authoritative STATE entries surfaced (highest precedence). */
  state: ForgeKnowledgeRecord[]
  excludedLessons: ForgeKnowledgeRecord[]
}

/**
 * Compile role-specific context from durable knowledge records applying the
 * precedence rules. It never stores or mutates STATE; it selects + orders.
 */
export function selectKnowledgeForRole(
  role: ForgeRole,
  records: ForgeKnowledgeRecord[],
): ForgeContextSelection {
  const allowed = ROLE_CLASS_POLICY[role]
  const state = records.filter((r) => r.class === 'STATE')

  const decisions = records.filter((r) => r.class === 'DECISION')

  const included: ForgeKnowledgeRecord[] = []
  const excludedLessons: ForgeKnowledgeRecord[] = []
  for (const r of records) {
    // STATE precedence: a stale historical observation (FACT/LESSON) cannot
    // outrank authoritative STATE; drop it when flagged stale.
    if (r.stale && (r.class === 'FACT' || r.class === 'LESSON')) continue
    // LESSON isolation: never surfaced to a role without LESSON reading rights,
    // and never allowed to override a current story DECISION.
    if (r.class === 'LESSON') {
      if (!allowed.has('LESSON') || storyHasDecision(decisions, r)) {
        excludedLessons.push(r)
        continue
      }
      included.push(r)
      continue
    }
    if (!allowed.has(r.class)) continue
    included.push(r)
  }

  return {
    role,
    included,
    state,
    excludedLessons,
  }
}

function storyHasDecision(
  decisions: ForgeKnowledgeRecord[],
  lesson: ForgeKnowledgeRecord,
): boolean {
  if (lesson.scope !== 'story') return false
  // A story-scoped lesson is suppressed when the same story already carries an
  // explicit DECISION (the current decision wins over historical learning).
  return decisions.some(
    (d) => d.class === 'DECISION' && d.scope === 'story' && d.storyId === lesson.storyId,
  )
}

/** Present STATE as the authoritative projection for a story. */
export function authoritativeState(
  storyId: string,
  stateRecords: ForgeKnowledgeRecord[],
): ForgeKnowledgeRecord[] {
  return stateRecords.filter((r) => r.class === 'STATE' && r.storyId === storyId)
}

