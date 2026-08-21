import { sql } from '../db/client'
import { isMissingRelation } from './engine-client'
import { collectAnomalies, type WorkflowAnomaly } from './anomaly-core'

// ---------------------------------------------------------------------------
// Workflow diagnostics foundation (CRM-14M — IT support).
//
// READ-ONLY. No mutation. These functions expose the live workflow engine
// state in a deterministic, typed shape for support/triage: deployed
// definitions, instances, tokens, engine tasks, jobs, commands, events,
// correlations, canonical tasks, and command receipts (including the
// poisoned 'pending' receipts that the engine cannot resolve).
// ---------------------------------------------------------------------------

export type DefinitionSummary = {
  definitionId: string
  key: string
  version: number
  name: string
  status: string
  instanceCount: number
  activeCount: number
}

export type InstanceSummary = {
  instanceId: string
  definitionKey: string
  definitionVersion: number
  subjectType: string | null
  subjectId: string | null
  status: string
  outcome: string | null
  startedAt: string
  endedAt: string | null
  activeTokenCount: number
  taskCount: number
  eventCount: number
}

export type TokenRow = {
  id: string
  parentTokenId: string | null
  nodeId: string
  status: string
  outcome: string | null
  required: boolean
}

export type TaskRow = {
  id: string
  tokenId: string | null
  name: string
  status: string
  candidates: string[]
  assignee: string | null
}

export type CommandReceiptRow = {
  commandId: string
  outcome: string
  aggregateId: string | null
  message: string | null
}

export type CorrelationRow = {
  workflowTaskId: string
  applicationTaskId: string | null
  applicationTaskStatus: string | null
  applicationTaskTitle: string | null
}

export type CommandRow = {
  commandId: string
  commandType: string
  nodeId: string
  outcome: string
  message: string | null
  receiptOutcome: string | null
}

export type InstanceDetail = InstanceSummary & {
  variables: Record<string, unknown> | null
  nodeLabels: Record<string, string>
  tokens: TokenRow[]
  tasks: TaskRow[]
  jobs: Array<{ id: string; type: string; status: string; dueAt: string | null }>
  events: Array<{ id: string; eventType: string; nodeId: string | null; actor: string | null }>
  correlations: CorrelationRow[]
  commands: CommandRow[]
}

export async function listDefinitions(): Promise<DefinitionSummary[]> {
  const rows = await sql`
    select
      d.id::text as definition_id,
      d.key,
      d.version,
      d.name,
      d.status,
      count(pi.id)::int as instance_count,
      count(pi.id) filter (where pi.status = 'active')::int as active_count
    from process_definitions d
    left join process_instances pi on pi.definition_id = d.id
    group by d.id, d.key, d.version, d.name, d.status
    order by d.key, d.version
  `
  return (rows as any[]).map((r) => ({
    definitionId: r.definition_id,
    key: r.key,
    version: r.version,
    name: r.name,
    status: r.status,
    instanceCount: r.instance_count,
    activeCount: r.active_count,
  }))
}

export async function listInstances(): Promise<InstanceSummary[]> {
  const rows = await sql`
    select
      pi.id::text as instance_id,
      pd.key,
      pd.version,
      pi.subject_type,
      pi.subject_id,
      pi.status,
      pi.outcome,
      pi.started_at::text as started_at,
      pi.ended_at::text as ended_at
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    order by pi.started_at desc
  `
  const instances = (rows as any[]).map((r) => ({
    instanceId: r.instance_id,
    definitionKey: r.key,
    definitionVersion: r.version,
    subjectType: r.subject_type ?? null,
    subjectId: r.subject_id ?? null,
    status: r.status,
    outcome: r.outcome ?? null,
    startedAt: r.started_at,
    endedAt: r.ended_at,
    activeTokenCount: 0,
    taskCount: 0,
    eventCount: 0,
  }))

  const [tokens, tasks, events] = await Promise.all([
    sql`select process_instance_id::text as pid, count(*)::int as c from tokens where status = 'active' group by process_instance_id`,
    sql`select process_instance_id::text as pid, count(*)::int as c from tasks group by process_instance_id`,
    sql`select process_instance_id::text as pid, count(*)::int as c from process_events group by process_instance_id`,
  ])

  const byToken = new Map((tokens as any[]).map((t) => [t.pid, t.c]))
  const byTask = new Map((tasks as any[]).map((t) => [t.pid, t.c]))
  const byEvent = new Map((events as any[]).map((t) => [t.pid, t.c]))

  for (const inst of instances) {
    inst.activeTokenCount = byToken.get(inst.instanceId) ?? 0
    inst.taskCount = byTask.get(inst.instanceId) ?? 0
    inst.eventCount = byEvent.get(inst.instanceId) ?? 0
  }
  return instances
}

export async function inspectInstance(instanceId: string): Promise<InstanceDetail | null> {
  const list = await listInstances()
  const summary = list.find((i) => i.instanceId === instanceId)
  if (!summary) return null

  const [tokens, tasks, jobs, events, correlations, meta] = await Promise.all([
    sql`
      select id::text, parent_token_id::text, node_id, status, outcome, required
      from tokens where process_instance_id = ${instanceId} order by created_at, id
    `,
    sql`
      select id::text, token_id::text, name, status, candidates, assignee
      from tasks where process_instance_id = ${instanceId} order by created_at
    `,
    sql`
      select id::text, type, status, due_at::text
      from jobs where process_instance_id = ${instanceId} order by created_at
    `,
    sql`
      select id::text, event_type, node_id, actor
      from process_events where process_instance_id = ${instanceId} order by id
    `,
    sql`
      select
        c.workflow_task_id,
        c.application_task_id::text as application_task_id,
        t.status as application_task_status,
        t.title as application_task_title
      from workflow_task_correlation c
      left join task t on t.id = c.application_task_id
      where c.workflow_task_id in (select id::text from tasks where process_instance_id = ${instanceId})
    `,
    sql`
      select pi.variables, pd.definition
      from process_instances pi
      join process_definitions pd on pd.id = pi.definition_id
      where pi.id = ${instanceId}
    `,
  ])

  const commands = await sql`
    select command_id, command_type, node_id, outcome, message
    from process_commands where process_instance_id = ${instanceId} order by id
  `

  const commandIds = (commands as any[]).map((c) => c.command_id)
  let receiptByCommand = new Map<string, CommandReceiptRow>()
  if (commandIds.length > 0) {
    const receiptRows = (await sql`
      select command_id, outcome, aggregate_id::text, message
      from workflow_command_receipt where command_id = any(${commandIds}::text[])
    `) as any[]
    receiptByCommand = new Map(
      receiptRows.map((r) => [
        r.command_id,
        {
          commandId: r.command_id,
          outcome: r.outcome,
          aggregateId: r.aggregate_id ?? null,
          message: r.message ?? null,
        },
      ])
    )
  }

  const metaRow = (meta as any[])[0]
  const variables = metaRow?.variables ?? null
  const nodeLabels = nodeLabelsFromDefinition(metaRow?.definition)

  return {
    ...summary,
    variables,
    nodeLabels,
    tokens: (tokens as any[]).map((t) => ({
      id: t.id,
      parentTokenId: t.parent_token_id ?? null,
      nodeId: t.node_id,
      status: t.status,
      outcome: t.outcome ?? null,
      required: t.required,
    })),
    tasks: (tasks as any[]).map((t) => ({
      id: t.id,
      tokenId: t.token_id ?? null,
      name: t.name,
      status: t.status,
      candidates: t.candidates ?? [],
      assignee: t.assignee ?? null,
    })),
    jobs: (jobs as any[]).map((j) => ({
      id: j.id,
      type: j.type,
      status: j.status,
      dueAt: j.due_at ?? null,
    })),
    events: (events as any[]).map((e) => ({
      id: String(e.id),
      eventType: e.event_type,
      nodeId: e.node_id ?? null,
      actor: e.actor ?? null,
    })),
    correlations: (correlations as any[]).map((c) => ({
      workflowTaskId: c.workflow_task_id,
      applicationTaskId: c.application_task_id ?? null,
      applicationTaskStatus: c.application_task_status ?? null,
      applicationTaskTitle: c.application_task_title ?? null,
    })),
    commands: (commands as any[]).map((c) => ({
      commandId: c.command_id,
      commandType: c.command_type,
      nodeId: c.node_id,
      outcome: c.outcome,
      message: c.message ?? null,
      receiptOutcome: receiptByCommand.get(c.command_id)?.outcome ?? null,
    })),
  }
}

export async function listCommandReceipts(): Promise<CommandReceiptRow[]> {
  const rows = await sql`
    select command_id, outcome, aggregate_id::text, message
    from workflow_command_receipt
    order by created_at
  `
  return (rows as any[]).map((r) => ({
    commandId: r.command_id,
    outcome: r.outcome,
    aggregateId: r.aggregate_id ?? null,
    message: r.message ?? null,
  }))
}

// ---------------------------------------------------------------------------
// Aggregate snapshot (CRM-14N — IT support summary + anomaly flags)
// ---------------------------------------------------------------------------

export type { WorkflowAnomaly } from './anomaly-core'

export type WorkflowDiagnosticsSummary = {
  definitionCount: number
  instanceTotal: number
  instanceActive: number
  instanceCompleted: number
  instanceFailed: number
  instanceOther: number
  readyEngineTasks: number
  correlatedOpenCanonicalTasks: number
  pendingJobs: number
  pendingReceipts: number
  anomalyCount: number
}

export type WorkflowInstanceRow = InstanceSummary & {
  propertyName: string | null
}

export type WorkflowDiagnosticsSnapshot = {
  configured: boolean
  summary: WorkflowDiagnosticsSummary
  definitions: DefinitionSummary[]
  instances: WorkflowInstanceRow[]
  anomalies: WorkflowAnomaly[]
}

const EMPTY_SUMMARY: WorkflowDiagnosticsSummary = {
  definitionCount: 0,
  instanceTotal: 0,
  instanceActive: 0,
  instanceCompleted: 0,
  instanceFailed: 0,
  instanceOther: 0,
  readyEngineTasks: 0,
  correlatedOpenCanonicalTasks: 0,
  pendingJobs: 0,
  pendingReceipts: 0,
  anomalyCount: 0,
}

export async function getWorkflowDiagnosticsSnapshot(): Promise<WorkflowDiagnosticsSnapshot> {
  try {
    return await buildWorkflowDiagnosticsSnapshot()
  } catch (err) {
    if (isMissingRelation(err)) {
      return {
        configured: false,
        summary: EMPTY_SUMMARY,
        definitions: [],
        instances: [],
        anomalies: [],
      }
    }
    throw err
  }
}

async function buildWorkflowDiagnosticsSnapshot(): Promise<WorkflowDiagnosticsSnapshot> {
  const [definitions, instances] = await Promise.all([
    listDefinitions(),
    listInstances(),
  ])

  const dealSubjectIds = instances
    .filter((i) => i.subjectType === 'deal' && i.subjectId)
    .map((i) => i.subjectId as string)

  const propertyBySubject = new Map<string, string | null>()
  if (dealSubjectIds.length > 0) {
    const dealRows = (await sql`
      select d.id::text as id, p.name as property_name
      from deal d
      join property p on p.id = d.property_id
      where d.id = any(${dealSubjectIds}::uuid[])
    `) as any[]
    for (const d of dealRows) propertyBySubject.set(d.id, d.property_name)
  }

  const instanceRows: WorkflowInstanceRow[] = instances.map((i) => ({
    ...i,
    propertyName: i.subjectId ? propertyBySubject.get(i.subjectId) ?? null : null,
  }))

  const [readyTasks, correlatedOpen, pendingJobs, pendingReceipts] = await Promise.all([
    sql`select count(*)::int as c from tasks where status in ('ready', 'reserved', 'in_progress')`,
    sql`
      select count(distinct c.application_task_id)::int as c
      from workflow_task_correlation c
      join task t on t.id = c.application_task_id
      where t.status = 'open'
    `,
    sql`select count(*)::int as c from jobs where status in ('pending', 'locked')`,
    sql`select count(*)::int as c from workflow_command_receipt where outcome = 'pending'`,
  ])

  const instanceActive = instances.filter((i) => i.status === 'active').length
  const instanceCompleted = instances.filter((i) => i.status === 'completed').length
  const instanceFailed = instances.filter(
    (i) => i.status === 'error' || i.outcome === 'failed'
  ).length

  const summary: WorkflowDiagnosticsSummary = {
    definitionCount: definitions.length,
    instanceTotal: instances.length,
    instanceActive,
    instanceCompleted,
    instanceFailed,
    instanceOther:
      instances.length - instanceActive - instanceCompleted - instanceFailed,
    readyEngineTasks: (readyTasks as any[])[0]?.c ?? 0,
    correlatedOpenCanonicalTasks: (correlatedOpen as any[])[0]?.c ?? 0,
    pendingJobs: (pendingJobs as any[])[0]?.c ?? 0,
    pendingReceipts: (pendingReceipts as any[])[0]?.c ?? 0,
    anomalyCount: 0,
  }

  const anomalies: WorkflowAnomaly[] = []
  await collectAnomalies(anomalies, instances, sql)
  summary.anomalyCount = anomalies.length

  return {
    configured: true,
    summary,
    definitions,
    instances: instanceRows,
    anomalies,
  }
}

function nodeLabelsFromDefinition(definition: unknown): Record<string, string> {
  const labels: Record<string, string> = {}
  if (definition && typeof definition === 'object') {
    const nodes = (definition as { nodes?: Record<string, { name?: string }> })
      .nodes
    if (nodes && typeof nodes === 'object') {
      for (const [id, node] of Object.entries(nodes)) {
        labels[id] = node?.name ?? id
      }
    }
  }
  return labels
}
