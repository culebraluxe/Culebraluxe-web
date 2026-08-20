import { sql } from '../db/client'
import { engineConfigured, engineSql } from './engine-client'
import { responsibilityFor } from './responsibility'
import { deadlineFor } from './deadlines'
import {
  TRANSACTION_CLOSE_V1_KEY,
  TRANSACTION_MILESTONE_BASE,
  TRANSACTION_MILESTONE_REQUIRED,
} from './definitions/transaction-close-v1'

// ---------------------------------------------------------------------------
// Portal-facing workflow read projections. The engine database and the
// CulebraLuxe database are separate, so results are joined in JS, never by
// raw engine row dumps.
// ---------------------------------------------------------------------------

export type WorkflowSummary = {
  instanceId: string
  workflowKey: string
  workflowVersion: number
  workflowName: string
  dealId: string
  propertyName: string | null
  status: string
  outcome: string | null
  currentNodes: string[]
  activeMilestones: string[]
  completedMilestones: string[]
  openTaskCount: number
  pendingTimerCount: number
  blockerCount: number
  responsibleParty: string | null
  nextExpectedAction: string | null
  startedAt: string
}

export type WorkflowEvent = {
  id: string
  eventType: string
  nodeId: string | null
  actor: string | null
  data: Record<string, unknown>
  createdAt: string
}

export type WorkflowDetail = WorkflowSummary & {
  events: WorkflowEvent[]
}

type EngineInstanceRow = {
  id: string
  subject_id: string | null
  status: string
  outcome: string | null
  started_at: string
  key: string
  version: number
  name: string
}

type EngineTokenRow = {
  process_instance_id: string
  node_id: string
  status: string
  outcome: string | null
}

function milestoneState(tokens: EngineTokenRow[]) {
  const active = new Set<string>()
  const completed = new Set<string>()
  const blockers = new Set<string>()
  for (const t of tokens) {
    const base = TRANSACTION_MILESTONE_BASE[t.node_id]
    if (!base) continue
    if (t.node_id.endsWith('_blocker')) {
      if (t.status === 'active') blockers.add(base)
      continue
    }
    if (t.status === 'active') active.add(base)
    else if (t.outcome === 'completed') completed.add(base)
  }
  return {
    active: [...active],
    completed: [...completed],
    blockers: [...blockers],
  }
}

function responsiblePartyFor(nodes: string[]): string | null {
  if (nodes.length === 0) return null
  return responsibilityFor(nodes[0]).owner
}

function nextExpectedActionFor(
  nodes: string[],
  milestones: ReturnType<typeof milestoneState>,
): string | null {
  if (nodes.length === 0) return null
  if (milestones.blockers.length > 0) {
    return `Resolve ${milestones.blockers.join(', ')} blocker`
  }
  const spec = responsibilityFor(nodes[0])
  const deadline = deadlineFor(nodes[0])
  return deadline.factSource ? `${spec.label} (${deadline.label})` : spec.label
}

export async function getWorkflowSummaries(): Promise<WorkflowSummary[]> {
  if (!engineConfigured()) return []

  const esql = engineSql()
  const instanceRows = await esql`
    select
      pi.id,
      pi.subject_id,
      pi.status,
      pi.outcome,
      pi.started_at::text as started_at,
      pd.key,
      pd.version,
      pd.name
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.subject_type = 'deal'
    order by pi.started_at desc
    limit 200
  `
  if (instanceRows.length === 0) return []

  const tokenRows = await esql`
    select t.process_instance_id, t.node_id, t.status, t.outcome
    from tokens t
    join process_instances pi on pi.id = t.process_instance_id
    where pi.subject_type = 'deal'
  `
  const taskRows = await esql`
    select t.process_instance_id, t.id
    from tasks t
    join process_instances pi on pi.id = t.process_instance_id
    where pi.subject_type = 'deal'
      and t.status in ('ready', 'reserved', 'in_progress')
  `
  const jobRows = await esql`
    select j.process_instance_id, j.id
    from jobs j
    join process_instances pi on pi.id = j.process_instance_id
    where pi.subject_type = 'deal' and j.status in ('pending', 'locked')
  `

  const tokensByInstance = new Map<string, EngineTokenRow[]>()
  for (const t of tokenRows as EngineTokenRow[]) {
    const list = tokensByInstance.get(t.process_instance_id) ?? []
    list.push(t)
    tokensByInstance.set(t.process_instance_id, list)
  }
  const taskCount = new Map<string, number>()
  for (const t of taskRows as { process_instance_id: string }[]) {
    taskCount.set(t.process_instance_id, (taskCount.get(t.process_instance_id) ?? 0) + 1)
  }
  const jobCount = new Map<string, number>()
  for (const j of jobRows as { process_instance_id: string }[]) {
    jobCount.set(j.process_instance_id, (jobCount.get(j.process_instance_id) ?? 0) + 1)
  }

  const dealIds = (instanceRows as EngineInstanceRow[])
    .map((r) => r.subject_id)
    .filter((id): id is string => id !== null)

  const dealRows = dealIds.length
    ? await sql`
        select d.id, p.name as property_name
        from deal d
        join property p on p.id = d.property_id
        where d.id = any(${dealIds}::uuid[])
      `
    : []
  const propertyByDeal = new Map<string, string | null>(
    (dealRows as { id: string; property_name: string }[]).map((d) => [
      d.id,
      d.property_name,
    ]),
  )

  return (instanceRows as EngineInstanceRow[]).map((r) => {
    const tokens = tokensByInstance.get(r.id) ?? []
    const currentNodes = tokens
      .filter((t) => t.status === 'active')
      .map((t) => t.node_id)
    const milestones = milestoneState(tokens)
    return {
      instanceId: r.id,
      workflowKey: r.key,
      workflowVersion: r.version,
      workflowName: r.name,
      dealId: r.subject_id ?? '',
      propertyName: propertyByDeal.get(r.subject_id ?? '') ?? null,
      status: r.status,
      outcome: r.outcome,
      currentNodes,
      activeMilestones: milestones.active,
      completedMilestones: milestones.completed,
      openTaskCount: taskCount.get(r.id) ?? 0,
      pendingTimerCount: jobCount.get(r.id) ?? 0,
      blockerCount: milestones.blockers.length,
      responsibleParty: responsiblePartyFor(currentNodes),
      nextExpectedAction: nextExpectedActionFor(currentNodes, milestones),
      startedAt: r.started_at,
    }
  })
}

export async function getWorkflowDetail(
  instanceId: string,
): Promise<WorkflowDetail | null> {
  if (!engineConfigured()) return null

  const summaries = await getWorkflowSummaries()
  const summary = summaries.find((s) => s.instanceId === instanceId)
  if (!summary) return null

  const esql = engineSql()
  const eventRows = await esql`
    select id, event_type, node_id, actor, data, created_at::text as created_at
    from process_events
    where process_instance_id = ${instanceId}
    order by id desc
    limit 100
  `

  return {
    ...summary,
    events: (eventRows as Array<{
      id: string
      event_type: string
      node_id: string | null
      actor: string | null
      data: Record<string, unknown>
      created_at: string
    }>).map((e) => ({
      id: String(e.id),
      eventType: e.event_type,
      nodeId: e.node_id,
      actor: e.actor,
      data: e.data ?? {},
      createdAt: e.created_at,
    })),
  }
}

/** Whether a workflow milestone applies to a given transaction (policy). */
export function isMilestoneRequired(milestoneId: string): boolean {
  return TRANSACTION_MILESTONE_REQUIRED[milestoneId] ?? false
}

export { TRANSACTION_CLOSE_V1_KEY }
