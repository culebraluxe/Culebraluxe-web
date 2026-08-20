import { sql } from '../db/client'
import { engineConfigured, engineSql, isMissingRelation } from './engine-client'
import { resolveResponsibility } from './responsibility'
import { deadlineLabelFor } from './deadlines'
import type { ProcessGraph, NodeDefinition } from '../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// Portal-facing workflow read projections.
//
// Story 128 — the read model is definition-driven, NOT hardcoded:
//   - definition key / version / name come from the deployed definition row
//   - node labels and descriptions come from the definition graph (which is
//     authored in XML); the XML node id IS the workflow state identity
//   - responsibility hints come from the graph and are resolved to an
//     operational owner by workflow_app (no node-id -> owner lookup table)
//   - timeline order comes from the definition's display-order metadata
//
// There is deliberately NO duplicate workflow-state enum / mapping layer.
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
  /** Active node ids (node id IS the workflow state identity). */
  currentNodes: string[]
  /** Labels of active human-work nodes (task nodes). */
  activeMilestones: string[]
  /** Labels of completed human-work nodes (task nodes). */
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
  /** Completed node ids (for the timeline). */
  completedNodes: string[]
  /** Node ids of active human-work nodes (paired with activeMilestones labels). */
  activeMilestoneNodeIds: string[]
  /** node id -> human-readable label from the definition graph. */
  nodeLabels: Record<string, string>
  /** node id -> description from the definition graph (where present). */
  nodeDescriptions: Record<string, string>
  /** node id -> responsibility hint from the definition graph (where present). */
  nodeResponsibility: Record<string, string>
  /** Ordered node ids for portal timeline presentation (definition metadata). */
  displayOrder: string[]
  /** Node ids that are conditional (target of a decision rule or optional fork branch). */
  optionalNodes: string[]
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
  description: string | null
  definition: ProcessGraph
}

type EngineTokenRow = {
  process_instance_id: string
  node_id: string
  status: string
  outcome: string | null
}

const CONTROL_TYPES = new Set([
  'start',
  'end',
  'decision',
  'fork',
  'join',
  'timer',
  'command',
])

/** Derive per-node presentation metadata from the definition graph. */
function graphViews(graph: ProcessGraph) {
  const nodeLabels: Record<string, string> = {}
  const nodeDescriptions: Record<string, string> = {}
  const nodeResponsibility: Record<string, string> = {}
  for (const id of Object.keys(graph.nodes)) {
    const node: NodeDefinition = graph.nodes[id]
    nodeLabels[id] = node.name ?? id
    if (node.description) nodeDescriptions[id] = node.description
    if (node.responsibility) nodeResponsibility[id] = node.responsibility
  }
  return { nodeLabels, nodeDescriptions, nodeResponsibility }
}

/**
 * Generic "is this node conditional?" analysis.
 * A node is optional/conditional when it is the target of a decision rule
 * (non-default decision transition) or of a fork branch marked required=false.
 */
function optionalNodeIds(graph: ProcessGraph): string[] {
  const optional = new Set<string>()
  for (const node of Object.values(graph.nodes)) {
    if (node.type === 'decision') {
      const defaultName = node.transitions?.[0]?.name
      for (const d of node.decisions ?? []) {
        const t = node.transitions?.find((x) => x.name === d.transition)
        if (t && t.name !== defaultName) optional.add(t.to)
      }
    }
    if (node.type === 'fork') {
      for (const t of node.transitions ?? []) {
        if (t.required === false) optional.add(t.to)
      }
    }
  }
  return [...optional]
}

function milestoneState(tokens: EngineTokenRow[], graph: ProcessGraph) {
  const active = new Set<string>()
  const activeNodeIds = new Set<string>()
  const completed = new Set<string>()
  const blockers = new Set<string>()
  const completedNodes = new Set<string>()
  for (const t of tokens) {
    const node = graph.nodes[t.node_id]
    if (!node) continue
    if (t.outcome === 'completed') completedNodes.add(t.node_id)
    if (CONTROL_TYPES.has(node.type)) continue
    const isBlocker = node.type === 'task' && t.node_id.endsWith('_blocker')
    if (t.status === 'active' && isBlocker) {
      blockers.add(node.name ?? t.node_id)
      continue
    }
    if (t.status === 'active') {
      active.add(node.name ?? t.node_id)
      activeNodeIds.add(t.node_id)
    } else if (t.outcome === 'completed') completed.add(node.name ?? t.node_id)
  }
  return { active, activeNodeIds, completed, blockers, completedNodes }
}

function responsiblePartyFor(nodes: string[], graph: ProcessGraph): string | null {
  for (const nodeId of nodes) {
    const hint = graph.nodes[nodeId]?.responsibility
    if (hint) return resolveResponsibility(hint).owner
  }
  return null
}

function nextExpectedActionFor(
  nodes: string[],
  graph: ProcessGraph,
  blockers: Set<string>,
): string | null {
  if (nodes.length === 0) return null
  if (blockers.size > 0) {
    return `Resolve ${[...blockers].join(', ')} blocker`
  }
  const nodeId = nodes[0]
  const node = graph.nodes[nodeId]
  if (!node) return null
  const label = node.name ?? nodeId
  const deadline = deadlineLabelFor(nodeId)
  const spec = resolveResponsibility(node.responsibility)
  return deadline ? `${spec.label} — ${label} (${deadline})` : `${spec.label} — ${label}`
}

export async function getWorkflowSummaries(): Promise<WorkflowSummary[]> {
  if (!engineConfigured()) return []

  const esql = engineSql()
  let instanceRows
  try {
    instanceRows = await esql`
      select
        pi.id,
        pi.subject_id,
        pi.status,
        pi.outcome,
        pi.started_at::text as started_at,
        pd.key,
        pd.version,
        pd.name,
        pd.description,
        pd.definition
      from process_instances pi
      join process_definitions pd on pd.id = pi.definition_id
      where pi.subject_type = 'deal'
      order by pi.started_at desc
      limit 200
    `
  } catch (err) {
    if (isMissingRelation(err)) return []
    throw err
  }
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
    const graph = r.definition ?? { startNodeId: '', nodes: {} }
    const tokens = tokensByInstance.get(r.id) ?? []
    const currentNodes = tokens
      .filter((t) => t.status === 'active')
      .map((t) => t.node_id)
    const milestones = milestoneState(tokens, graph)
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
      activeMilestones: [...milestones.active],
      completedMilestones: [...milestones.completed],
      openTaskCount: taskCount.get(r.id) ?? 0,
      pendingTimerCount: jobCount.get(r.id) ?? 0,
      blockerCount: milestones.blockers.size,
      responsibleParty: responsiblePartyFor(currentNodes, graph),
      nextExpectedAction: nextExpectedActionFor(currentNodes, graph, milestones.blockers),
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
  let eventRows
  try {
    eventRows = await esql`
      select id, event_type, node_id, actor, data, created_at::text as created_at
      from process_events
      where process_instance_id = ${instanceId}
      order by id desc
      limit 100
    `
  } catch (err) {
    if (isMissingRelation(err)) return null
    throw err
  }

  // Reload the graph for this instance to attach node presentation metadata.
  const graphRows = await esql`
    select pd.definition
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.id = ${instanceId}
    limit 1
  `
  const graph: ProcessGraph =
    (graphRows[0] as { definition: ProcessGraph } | undefined)?.definition ?? {
      startNodeId: '',
      nodes: {},
    }
  const views = graphViews(graph)

  const tokenRows = await esql`
    select node_id, status, outcome
    from tokens
    where process_instance_id = ${instanceId}
  `
  const completedNodes = (tokenRows as EngineTokenRow[])
    .filter((t) => t.outcome === 'completed')
    .map((t) => t.node_id)
  const milestones = milestoneState(tokenRows as EngineTokenRow[], graph)

  return {
    ...summary,
    completedNodes,
    activeMilestoneNodeIds: [...milestones.activeNodeIds],
    nodeLabels: views.nodeLabels,
    nodeDescriptions: views.nodeDescriptions,
    nodeResponsibility: views.nodeResponsibility,
    displayOrder: graph.displayOrder ?? [],
    optionalNodes: optionalNodeIds(graph),
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
