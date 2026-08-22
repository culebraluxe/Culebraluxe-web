// ---------------------------------------------------------------------------
// Task materialization reconciliation — the V1 caller for materializeEngineTask.
//
// Discovers active engine human tasks and materializes each exactly once into
// a canonical CulebraLuxe operational task via deterministic correlation. Safe
// on repeated invocation. No scheduler here.
//
// CRM-15 — SME-aware: for each engine task the deployed definition graph's
// responsibility hint is read (task -> token node_id -> graph node) and the
// canonical task is orchestrated through sme-orchestration, so an external-SME
// task is materialized ADDRESSED TO the responsible SME participant
// (task.person_id) when one is recorded on the deal. An unrecorded SME never
// blocks materialization (SME-less task) — the workflow must not depend on a
// participant row existing.
// ---------------------------------------------------------------------------

import type { ProcessGraph } from '../workflow_engine/lib/workflow/types'
import type { SmeParticipantCandidate } from './sme-orchestration'

export type EngineTaskView = {
  workflowTaskId: string
  title: string
  subjectType: string
  subjectId: string
  dealId: string | null
  /** Responsibility hint from the deployed definition graph node (CRM-15). */
  responsibilityHint?: string
}

export async function reconcileTaskMaterializationCore(
  tasks: EngineTaskView[],
  materialize: (
    t: EngineTaskView,
  ) => Promise<{ applicationTaskId: string; created: boolean }>,
): Promise<{ materialized: number; skipped: number }> {
  let materialized = 0
  let skipped = 0
  for (const t of tasks) {
    const r = await materialize(t)
    if (r.created) materialized += 1
    else skipped += 1
  }
  return { materialized, skipped }
}

type ReconciliationTaskRow = {
  workflow_task_id: string
  title: string
  subject_type: string
  subject_id: string
  deal_id: string
  node_id: string
  definition: ProcessGraph
}

type ParticipantRow = {
  deal_id: string
  id: string
  role: string
  role_label: string | null
  person_id: string | null
  user_id: string | null
  active: boolean
}

export async function reconcileTaskMaterialization(): Promise<{
  materialized: number
  skipped: number
}> {
  const { engineConfigured, engineSql } = await import('./engine-client')
  if (!engineConfigured()) return { materialized: 0, skipped: 0 }

  const { sql } = await import('../db/client')
  const { materializeEngineTask } = await import('./task-materialization')
  const { orchestrateSmeTaskCore } = await import('./sme-orchestration')
  const esql = engineSql()

  const rows = (await esql`
    select
      t.id as workflow_task_id,
      t.name as title,
      pi.subject_type,
      pi.subject_id,
      pi.subject_id as deal_id,
      tok.node_id,
      pd.definition
    from tasks t
    join process_instances pi on pi.id = t.process_instance_id
    join tokens tok on tok.id = t.token_id
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'deal'
      and t.status in ('ready', 'reserved', 'in_progress')
  `) as ReconciliationTaskRow[]

  const tasks: EngineTaskView[] = rows.map((r) => ({
    workflowTaskId: r.workflow_task_id,
    title: r.title,
    subjectType: r.subject_type,
    subjectId: r.subject_id,
    dealId: r.deal_id,
    // CRM-15 — the responsibility hint comes from the deployed definition
    // graph (the XML node id IS the workflow state identity; the engine
    // never resolves the hint, so we read it here for the application).
    responsibilityHint: r.definition?.nodes?.[r.node_id]?.responsibility,
  }))

  // Load participants once per deal (resolution considers only active rows;
  // inactive rows are still returned for transparency).
  const dealIds = [
    ...new Set(
      tasks
        .map((t) => t.dealId)
        .filter((d): d is string => d !== null && d.length > 0),
    ),
  ]
  const participantsByDeal = new Map<string, SmeParticipantCandidate[]>()
  if (dealIds.length > 0) {
    const participantRows = (await sql`
      select deal_id, id, role, role_label, person_id, user_id, active
      from deal_participant
      where deal_id = any(${dealIds}::uuid[])
      order by started_at asc, created_at asc
    `) as ParticipantRow[]
    for (const r of participantRows) {
      const list = participantsByDeal.get(r.deal_id) ?? []
      list.push({
        id: r.id,
        role: r.role,
        roleLabel: r.role_label,
        personId: r.person_id,
        userId: r.user_id,
        active: r.active,
      })
      participantsByDeal.set(r.deal_id, list)
    }
  }

  return reconcileTaskMaterializationCore(tasks, (t) =>
    orchestrateSmeTaskCore(
      {
        workflowTaskId: t.workflowTaskId,
        title: t.title,
        subjectType: t.subjectType,
        subjectId: t.subjectId,
        dealId: t.dealId ?? undefined,
        responsibilityHint: t.responsibilityHint,
      },
      {
        participants: (t.dealId && participantsByDeal.get(t.dealId)) || [],
        materialize: materializeEngineTask,
      },
    ),
  )
}
