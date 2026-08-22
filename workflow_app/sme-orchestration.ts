// ---------------------------------------------------------------------------
// CRM-15 — External SME orchestration seam.
//
// The RE_supermodel declares external-SME responsibilities on task nodes
// (responsibility="appraiser" / "inspector" / "lender" / "title_company" /
// "notario" / "other_sme"). The engine treats them as free-string metadata it
// mirrors into candidateGroups; it NEVER resolves them to application
// identity. This module is the application-side orchestration boundary that
// makes real external SMEs first-class participants of the closing workflow:
//
//   resolveSmeParticipant — resolve an XML responsibility hint to the ACTUAL
//       responsible SME: the active canonical deal_participant whose role
//       matches (long tail: role='other' + role_label; structural:
//       role='client'/'seller'). Never invents a participant — no match is a
//       typed 'none' result, never a fabricated identity.
//   orchestrateSmeTask    — resolve the SME and materialize the engine task
//       as a canonical CulebraLuxe task ADDRESSED TO the SME
//       (task.person_id = the SME person), preserving the 1:1 workflow-task
//       correlation idempotency of materializeEngineTask. A task whose SME is
//       not yet recorded still materializes (SME-less), so an unrecorded
//       participant can never block the closing orchestration.
//   completeSmeTask path  — the SME completion path is the existing
//       task-completion seam (completeWorkflowTask: canonical task -> engine
//       task -> deployed transition), which advances the closing
//       orchestration; this module composes it, never duplicates it.
//
// No provider integration (appraiser API, lender portal, ...) lives here:
// a connector behind this boundary is a separately reviewed story. This is
// the canonical orchestration boundary between the workflow and real external
// SMEs (responsibility.ts Story 117 / CRM-13 reconciliation).
// ---------------------------------------------------------------------------

import type { QueryExecutor } from '../db/query-executor'
import type { MaterializeTaskInput, MaterializeTaskResult } from './task-materialization'
import {
  resolveResponsibility,
  resolveParticipantTarget,
  type ParticipantResolution,
  type ResponsibilitySpec,
} from './responsibility'

/** Canonical deal_participant row shape the orchestration resolves against. */
export type SmeParticipantCandidate = {
  id: string
  /** deal_participant.role: 'client' | 'owner' | 'seller' | 'other'. */
  role: string
  /** Long-tail label for role='other' (lender, appraiser, ...), else null. */
  roleLabel: string | null
  personId: string | null
  userId: string | null
  /** Only active participants are eligible (canonical read semantics). */
  active: boolean
}

export type SmeResolution =
  | {
      kind: 'sme'
      participant: SmeParticipantCandidate
      spec: ResponsibilitySpec
      target: ParticipantResolution
    }
  | { kind: 'none'; reason: 'no_hint' | 'no_target' | 'no_participant' }

/**
 * Pure resolution: an XML responsibility hint -> the actual responsible SME
 * participant. The hint is a SEPARATE concept from the participant taxonomy
 * (responsibility.ts): buyer/seller resolve to the structural
 * role='client'/'seller' row; SME hints (appraiser, inspector, lender,
 * notario, title_company) resolve to the long-tail role='other' +
 * role_label row (case-insensitive). brokerage / other_sme / unknown hints
 * have no participant target. A missing or inactive participant is a typed
 * 'no_participant' — the workflow never fabricates an SME.
 */
export function resolveSmeParticipant(
  participants: SmeParticipantCandidate[],
  hint: string | undefined | null,
): SmeResolution {
  if (!hint) return { kind: 'none', reason: 'no_hint' }
  const spec = resolveResponsibility(hint)
  const target = resolveParticipantTarget(spec)
  if (!target) return { kind: 'none', reason: 'no_target' }

  const eligible = participants.filter((p) => p.active)
  let match: SmeParticipantCandidate | null = null
  if (target.kind === 'structural') {
    match = eligible.find((p) => p.role === target.role) ?? null
  } else {
    const label = target.roleLabel.toLowerCase()
    match =
      eligible.find(
        (p) => p.role === 'other' && (p.roleLabel ?? '').toLowerCase() === label,
      ) ?? null
  }
  if (!match) return { kind: 'none', reason: 'no_participant' }
  return { kind: 'sme', participant: match, spec, target }
}

export type OrchestrateSmeTaskInput = {
  workflowTaskId: string
  title: string
  detail?: string
  subjectType: string
  subjectId: string
  dealId?: string
  propertyId?: string
  dueAt?: string
  /** Responsibility hint from the deployed definition graph node, if any. */
  responsibilityHint?: string
}

export type OrchestrateSmeTaskDeps = {
  /** Active deal participants used to resolve the responsible SME. */
  participants: SmeParticipantCandidate[]
  /** The existing materialization seam (materializeEngineTask in production). */
  materialize: (input: MaterializeTaskInput) => Promise<MaterializeTaskResult>
}

export type OrchestrateSmeTaskResult = MaterializeTaskResult & {
  /** How the responsibility hint resolved (or why it did not). */
  sme: SmeResolution
}

/**
 * Pure orchestration core: resolve the responsible SME for the task node and
 * materialize the canonical task through the existing seam with the SME
 * person attached. Idempotent exactly like materializeEngineTaskCore: a retry
 * returns the existing correlation without creating a duplicate task. An
 * unresolvable SME is reported (never invented) and the task still
 * materializes so the workflow is never blocked by an unrecorded participant.
 */
export async function orchestrateSmeTaskCore(
  input: OrchestrateSmeTaskInput,
  deps: OrchestrateSmeTaskDeps,
): Promise<OrchestrateSmeTaskResult> {
  const sme = resolveSmeParticipant(deps.participants, input.responsibilityHint)
  const result = await deps.materialize({
    workflowTaskId: input.workflowTaskId,
    title: input.title,
    detail: input.detail,
    subjectType: input.subjectType,
    subjectId: input.subjectId,
    dealId: input.dealId,
    propertyId: input.propertyId,
    dueAt: input.dueAt,
    // The canonical task is ADDRESSED TO the responsible SME when one is
    // recorded (task.person_id = the SME person).
    personId:
      sme.kind === 'sme' && sme.participant.personId
        ? sme.participant.personId
        : undefined,
  })
  return { ...result, sme }
}

type ParticipantRow = {
  id: string
  role: string
  role_label: string | null
  person_id: string | null
  user_id: string | null
  active: boolean
}

function mapParticipantRow(r: ParticipantRow): SmeParticipantCandidate {
  return {
    id: r.id,
    role: r.role,
    roleLabel: r.role_label,
    personId: r.person_id,
    userId: r.user_id,
    active: r.active,
  }
}

// Lazy default executor (mirrors db/deal-participants.ts) so importing this
// module never requires a DATABASE_URL; tests inject an in-memory fake.
let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('../db/client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

/** Active (and inactive, for resolution correctness) participants of a deal. */
export async function listDealParticipants(
  dealId: string,
  execute?: QueryExecutor,
): Promise<SmeParticipantCandidate[]> {
  const run = execute ?? (await executor())
  const rows = (await run`
    select id, role, role_label, person_id, user_id, active
    from deal_participant
    where deal_id = ${dealId}
    order by started_at asc, created_at asc
  `) as ParticipantRow[]
  return rows.map(mapParticipantRow)
}

/**
 * Production wiring: load the deal's participants and orchestrate the SME
 * task through the existing materializeEngineTask seam.
 */
export async function orchestrateSmeTask(
  input: OrchestrateSmeTaskInput,
): Promise<OrchestrateSmeTaskResult> {
  const { materializeEngineTask } = await import('./task-materialization')
  const participants = input.dealId ? await listDealParticipants(input.dealId) : []
  return orchestrateSmeTaskCore(input, {
    participants,
    materialize: materializeEngineTask,
  })
}
