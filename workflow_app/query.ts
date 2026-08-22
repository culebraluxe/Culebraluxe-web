import { engineSql, isMissingRelation } from './engine-client'
import {
  classifyInstanceHealth,
  type WorkflowAnomaly,
  type WorkflowHealth,
} from './anomaly-core'
import type { QueryExecutor } from '../db/query-executor'
import type { ProcessGraph, NodeDefinition } from '../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// ENG-15 — Workflow Instance Query / History / Health.
//
// ONE coherent generic read contract for a workflow instance: where it is,
// what is active / waiting / retrying / failed, what tasks / jobs / timers /
// commands exist, what happened historically, and its health classification.
//
// This module consolidates the semantics that previously lived apart:
//   - read-service graph presentation (node labels/descriptions/responsibility,
//     milestone classification, display order, optional-node analysis) — the
//     shared helpers below are the single implementation, imported by
//     read-service so portal views and the generic contract cannot drift
//   - diagnostics instance inspection (tokens / tasks / jobs / commands /
//     correlations / command receipts)
//   - anomaly-core operational anomaly rules, projected per instance and
//     turned into the stuck/healthy classification via
//     classifyInstanceHealth (the single classification source)
//   - process_events history semantics (append-only event log, newest first)
//
// DELIBERATELY NOT event-sourced and NOT a fourth projection: every field is
// derived at read time from existing canonical runtime state
// (process_instances, tokens, tasks, jobs, process_commands,
// workflow_task_correlation, workflow_command_receipt, process_events). No
// table, view, or denormalized cache is created or written.
//
// GENERIC: keyed by process instance id, independent of subject type — it
// serves the real-estate workflow UI (deal subjects) today and the future
// SDLC dogfood chevron/process visualization (any subject type) unchanged.
// The injected `execute` handle keeps the contract durably testable with an
// in-memory fake (same pattern as anomaly-core) and defaults to the live
// engine handle for the portal/operator path.
// ---------------------------------------------------------------------------

export type WorkflowTaskView = {
  id: string
  name: string
  status: string
  /** The workflow-state node the task belongs to (node id IS the state identity). */
  nodeId: string | null
  /** Engine token this task is anchored to. */
  tokenId: string | null
  assignee: string | null
  candidates: string[]
  dueDate: string | null
}

export type WorkflowJobView = {
  id: string
  type: string
  status: string
  dueAt: string | null
  attempts: number
  maxAttempts: number
  lastError: string | null
}

export type WorkflowCommandView = {
  commandId: string
  commandType: string
  nodeId: string
  outcome: string
  message: string | null
  receiptOutcome: string | null
}

export type WorkflowCorrelationView = {
  workflowTaskId: string
  applicationTaskId: string | null
  applicationTaskStatus: string | null
  applicationTaskTitle: string | null
}

export type WorkflowEventView = {
  id: string
  eventType: string
  nodeId: string | null
  actor: string | null
  data: Record<string, unknown>
  createdAt: string
}

export type WorkflowMilestone = {
  nodeId: string
  label: string
  responsibility: string | null
}

/**
 * Work-state rollup of actionable work for one instance.
 *
 * Buckets are mutually exclusive and non-overlapping:
 *   active   — executing right now: active tokens, locked jobs,
 *              reserved/in_progress tasks
 *   waiting  — normal waiting: ready tasks and pending jobs (attempts = 0,
 *              includes timers awaiting their due time)
 *   retrying — failed and scheduled for retry: pending jobs with attempts > 0
 *              (engine backoff; `job.retry_scheduled`)
 *   failed   — terminal failures: failed tasks, failed jobs, and commands
 *              whose outcome is not success
 */
export type WorkflowStateRollup = {
  active: number
  waiting: number
  retrying: number
  failed: number
  activeTokens: number
  readyTasks: number
  inProgressTasks: number
  lockedJobs: number
  pendingJobs: number
  retryingJobs: number
  failedTasks: number
  failedJobs: number
  failedCommands: number
}

export type WorkflowInstanceQuery = {
  /** Where the instance is (canonical position + definition presentation). */
  instance: {
    instanceId: string
    definitionKey: string
    definitionVersion: number
    definitionName: string
    subjectType: string | null
    subjectId: string | null
    status: string
    outcome: string | null
    variables: Record<string, unknown> | null
    startedAt: string
    endedAt: string | null
    /** Active token node ids (node id IS the workflow state identity). */
    currentNodeIds: string[]
    /** Token node ids with outcome 'completed'. */
    completedNodeIds: string[]
    /** Active human-work nodes (non-control, non-blocker task nodes). */
    activeMilestones: WorkflowMilestone[]
    /** Labels of active `_blocker` task nodes. */
    blockerLabels: string[]
    /** node id -> human-readable label from the definition graph. */
    nodeLabels: Record<string, string>
    /** node id -> description from the definition graph (where present). */
    nodeDescriptions: Record<string, string>
    /** node id -> responsibility hint from the definition graph (where present). */
    nodeResponsibility: Record<string, string>
    /** Ordered node ids for timeline presentation (definition metadata). */
    displayOrder: string[]
    /** Node ids that are conditional (decision target or optional fork branch). */
    optionalNodes: string[]
  }
  /** What is active / waiting / retrying / failed. */
  state: WorkflowStateRollup
  /** What tasks / jobs / timers / commands / correlations exist. */
  work: {
    tasks: WorkflowTaskView[]
    jobs: WorkflowJobView[]
    /** Subset of jobs with type 'timer' (pending/locked/completed/failed). */
    timers: WorkflowJobView[]
    commands: WorkflowCommandView[]
    correlations: WorkflowCorrelationView[]
  }
  /** What happened historically (process_events, newest first). */
  history: WorkflowEventView[]
  /** Health classification tied to the operational anomaly rules. */
  health: WorkflowHealth
}

export type { WorkflowAnomaly, WorkflowHealth } from './anomaly-core'
export { classifyInstanceHealth } from './anomaly-core'

// ---------------------------------------------------------------------------
// Shared definition-graph presentation semantics (single implementation —
// read-service imports these; do not fork them).
// ---------------------------------------------------------------------------

export const CONTROL_TYPES = new Set([
  'start',
  'end',
  'decision',
  'fork',
  'join',
  'timer',
  'command',
])

/** Derive per-node presentation metadata from the definition graph. */
export function graphViews(graph: ProcessGraph) {
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
export function optionalNodeIds(graph: ProcessGraph): string[] {
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

/** Minimal token fields the milestone classifier needs (structural). */
export type MilestoneTokenRow = {
  node_id: string
  status: string
  outcome: string | null
}

/** Token row for the per-instance query (adds the token identity). */
export type EngineTokenRow = MilestoneTokenRow & {
  id: string
}

export function milestoneState(tokens: MilestoneTokenRow[], graph: ProcessGraph) {
  const active = new Set<string>()
  const activeNodeIds = new Set<string>()
  const completed = new Set<string>()
  const blockers = new Set<string>()
  const blockerNodeIds = new Set<string>()
  const completedNodes = new Set<string>()
  for (const t of tokens) {
    const node = graph.nodes[t.node_id]
    if (!node) continue
    if (t.outcome === 'completed') completedNodes.add(t.node_id)
    if (CONTROL_TYPES.has(node.type)) continue
    const isBlocker = node.type === 'task' && t.node_id.endsWith('_blocker')
    if (t.status === 'active' && isBlocker) {
      blockers.add(node.name ?? t.node_id)
      blockerNodeIds.add(t.node_id)
      continue
    }
    if (t.status === 'active') {
      active.add(node.name ?? t.node_id)
      activeNodeIds.add(t.node_id)
    } else if (t.outcome === 'completed') completed.add(node.name ?? t.node_id)
  }
  return {
    active,
    activeNodeIds,
    completed,
    blockers,
    blockerNodeIds,
    completedNodes,
  }
}

// ---------------------------------------------------------------------------
// Instance-scoped anomaly collection — the per-instance projection of the
// operational anomaly rules (anomaly-core). Same kinds, same messages; each
// rule is scoped to the queried instance instead of a global sweep.
// ---------------------------------------------------------------------------

type InstanceRow = {
  instance_id: string
  subject_type: string | null
  subject_id: string | null
  status: string
  outcome: string | null
  variables: unknown
  started_at: string
  ended_at: string | null
  key: string
  version: number
  name: string
  definition: ProcessGraph
}

type JobRow = {
  id: string
  token_id: string | null
  type: string
  status: string
  due_at: string | null
  attempts: number
  max_attempts: number
  last_error: string | null
  locked_by: string | null
  locked_until: string | null
}

function collectInstanceAnomalies(
  anomalies: WorkflowAnomaly[],
  instance: InstanceRow,
  tokens: EngineTokenRow[],
  jobs: JobRow[],
  commands: Array<{ commandId: string; outcome: string }>,
  receipts: Array<{ command_id: string; outcome: string; message: string | null }>,
  now: () => Date,
): void {
  const keyLabel = `${instance.key} v${instance.version}`

  // failed-process — status error or failed outcome (rule 1, anomaly-core).
  if (instance.status === 'error' || instance.outcome === 'failed') {
    anomalies.push({
      kind: 'failed-process',
      severity: 'critical',
      instanceId: instance.instance_id,
      subjectId: instance.subject_id,
      message:
        instance.status === 'error'
          ? `Process ${instance.instance_id} is in error state (${keyLabel})`
          : `Process ${instance.instance_id} terminated with outcome '${instance.outcome}' (${keyLabel})`,
    })
  }

  // error-instance-missing-outcome — 'error' status with no terminal outcome
  // (rule 11, anomaly-core).
  if (instance.status === 'error' && instance.outcome === null) {
    anomalies.push({
      kind: 'error-instance-missing-outcome',
      severity: 'critical',
      instanceId: instance.instance_id,
      subjectId: null,
      message: `Process ${instance.instance_id} (${keyLabel}) is 'error' with no terminal outcome`,
    })
  }

  const activeTokens = tokens.filter((t) => t.status === 'active')
  const pendingOrLockedJobs = jobs.filter((j) =>
    j.status === 'pending' || j.status === 'locked'
  )

  // wedged-instance — active with no active tokens and no pending work (rule 9).
  if (instance.status === 'active' && activeTokens.length === 0 && pendingOrLockedJobs.length === 0) {
    anomalies.push({
      kind: 'wedged-instance',
      severity: 'warning',
      instanceId: instance.instance_id,
      subjectId: null,
      message: `Process ${instance.instance_id} (${keyLabel}) is active but has no active tokens or pending work — was never terminalized`,
    })
  }

  // orphan-token — active token on a terminal instance (rule 10, scoped).
  if (instance.status !== 'active') {
    for (const t of activeTokens) {
      anomalies.push({
        kind: 'orphan-token',
        severity: 'warning',
        instanceId: instance.instance_id,
        subjectId: null,
        message: `Token ${t.id} at '${t.node_id}' is active but its process is missing or not active`,
      })
    }
  }

  const tokenOutcomeById = new Map(tokens.map((t) => [t.id, t.outcome]))

  // stale-locked-job — locked job whose lease has expired (rule 8, scoped).
  const nowMs = now().getTime()
  for (const j of jobs) {
    if (j.status === 'locked' && j.locked_until && new Date(j.locked_until).getTime() < nowMs) {
      anomalies.push({
        kind: 'stale-locked-job',
        severity: 'warning',
        instanceId: instance.instance_id,
        subjectId: null,
        message: `Job ${j.id} (${j.type}) is locked by ${j.locked_by} past its lease (${j.locked_until})`,
      })
    }
  }

  // open-job-on-closed-token — open job whose token already completed/skipped
  // (rule 6, scoped).
  for (const j of pendingOrLockedJobs) {
    const outcome = j.token_id ? tokenOutcomeById.get(j.token_id) : null
    if (outcome === 'completed' || outcome === 'skipped') {
      anomalies.push({
        kind: 'open-job-on-closed-token',
        severity: 'warning',
        instanceId: instance.instance_id,
        subjectId: null,
        message: `Job ${j.id} (${j.type}) is ${j.status} but token ${j.token_id} is already ${outcome}`,
      })
    }
  }

  // pending-receipt — poisoned command receipt for one of this instance's
  // commands (rule 2, scoped via process_commands).
  const commandIds = new Set(commands.map((c) => c.commandId))
  for (const r of receipts) {
    if (r.outcome === 'pending' && commandIds.has(r.command_id)) {
      anomalies.push({
        kind: 'pending-receipt',
        severity: 'critical',
        instanceId: instance.instance_id,
        subjectId: null,
        message: `Command receipt ${r.command_id} is stuck 'pending'${r.message ? ` — ${r.message}` : ''}`,
      })
    }
  }
}

function buildStateRollup(
  tokens: EngineTokenRow[],
  tasks: Array<{ status: string }>,
  jobs: JobRow[],
  commands: Array<{ outcome: string }>,
): WorkflowStateRollup {
  const activeTokens = tokens.filter((t) => t.status === 'active').length
  const readyTasks = tasks.filter((t) => t.status === 'ready').length
  const inProgressTasks = tasks.filter(
    (t) => t.status === 'reserved' || t.status === 'in_progress'
  ).length
  const failedTasks = tasks.filter((t) => t.status === 'failed').length
  const lockedJobs = jobs.filter((j) => j.status === 'locked').length
  const pendingJobs = jobs.filter(
    (j) => j.status === 'pending' && j.attempts === 0
  ).length
  const retryingJobs = jobs.filter(
    (j) => j.status === 'pending' && j.attempts > 0
  ).length
  const failedJobs = jobs.filter((j) => j.status === 'failed').length
  const failedCommands = commands.filter((c) => c.outcome !== 'success').length
  return {
    active: activeTokens + lockedJobs + inProgressTasks,
    waiting: readyTasks + pendingJobs,
    retrying: retryingJobs,
    failed: failedTasks + failedJobs + failedCommands,
    activeTokens,
    readyTasks,
    inProgressTasks,
    lockedJobs,
    pendingJobs,
    retryingJobs,
    failedTasks,
    failedJobs,
    failedCommands,
  }
}

// ---------------------------------------------------------------------------
// The query.
// ---------------------------------------------------------------------------

export async function queryWorkflowInstance(
  instanceId: string,
  execute: QueryExecutor = engineSql(),
  now: () => Date = () => new Date(),
): Promise<WorkflowInstanceQuery | null> {
  let instanceRows
  try {
    instanceRows = await execute`
      select
        pi.id::text as instance_id,
        pi.subject_type,
        pi.subject_id::text as subject_id,
        pi.status,
        pi.outcome,
        pi.variables,
        pi.started_at::text as started_at,
        pi.ended_at::text as ended_at,
        pd.key,
        pd.version,
        pd.name,
        pd.definition
      from process_instances pi
      join process_definitions pd on pd.id = pi.definition_id
      where pi.id = ${instanceId}
      limit 1
    `
  } catch (err) {
    if (isMissingRelation(err)) return null
    throw err
  }
  const instanceRow = instanceRows[0] as InstanceRow | undefined
  if (!instanceRow) return null

  const graph: ProcessGraph = instanceRow.definition ?? {
    startNodeId: '',
    nodes: {},
  }

  const [tokenRows, taskRows, jobRows, commandRows, eventRows, correlationRows] =
    await Promise.all([
      execute`
        select id::text as id, node_id, status, outcome
        from tokens
        where process_instance_id = ${instanceId}
      `,
      execute`
        select
          id::text as id,
          token_id::text as token_id,
          name,
          status,
          candidates,
          assignee,
          due_date::text as due_date
        from tasks
        where process_instance_id = ${instanceId}
        order by created_at
      `,
      execute`
        select
          id::text as id,
          token_id::text as token_id,
          type,
          status,
          due_at::text as due_at,
          attempts,
          max_attempts,
          last_error,
          locked_by,
          locked_until::text as locked_until
        from jobs
        where process_instance_id = ${instanceId}
        order by created_at
      `,
      execute`
        select command_id, command_type, node_id, outcome, message
        from process_commands
        where process_instance_id = ${instanceId}
        order by id
      `,
      execute`
        select id, event_type, node_id, actor, data, created_at::text as created_at
        from process_events
        where process_instance_id = ${instanceId}
        order by id desc
        limit 100
      `,
      execute`
        select
          c.workflow_task_id,
          c.application_task_id::text as application_task_id,
          t.status as application_task_status,
          t.title as application_task_title
        from workflow_task_correlation c
        left join task t on t.id = c.application_task_id
        where c.workflow_task_id in (
          select id::text from tasks where process_instance_id = ${instanceId}
        )
      `,
    ])

  const tokens = tokenRows as EngineTokenRow[]
  const nodeIdByToken = new Map(tokens.map((t) => [t.id, t.node_id]))
  const tasks = taskRows as Array<{
    id: string
    token_id: string | null
    name: string
    status: string
    candidates: string[] | null
    assignee: string | null
    due_date: string | null
  }>
  const jobs = jobRows as JobRow[]
  const commands = commandRows as Array<{
    command_id: string
    command_type: string
    node_id: string
    outcome: string
    message: string | null
  }>

  // Command receipts for this instance's commands (outcome mirror for
  // operators; 'pending' receipts drive the pending-receipt anomaly).
  let receiptRows: Array<{ command_id: string; outcome: string; message: string | null }> =
    []
  const commandIds = commands.map((c) => c.command_id)
  if (commandIds.length > 0) {
    receiptRows = (await execute`
      select command_id, outcome, message
      from workflow_command_receipt
      where command_id = any(${commandIds}::text[])
    `) as Array<{ command_id: string; outcome: string; message: string | null }>
  }

  const views = graphViews(graph)
  const milestones = milestoneState(tokens, graph)

  const anomalies: WorkflowAnomaly[] = []
  collectInstanceAnomalies(
    anomalies,
    instanceRow,
    tokens,
    jobs,
    commands.map((c) => ({ commandId: c.command_id, outcome: c.outcome })),
    receiptRows,
    now,
  )
  const health = classifyInstanceHealth({
    status: instanceRow.status,
    outcome: instanceRow.outcome,
    anomalies,
  })

  const jobViews: WorkflowJobView[] = jobs.map((j) => ({
    id: j.id,
    type: j.type,
    status: j.status,
    dueAt: j.due_at,
    attempts: j.attempts ?? 0,
    maxAttempts: j.max_attempts ?? 0,
    lastError: j.last_error,
  }))

  return {
    instance: {
      instanceId: instanceRow.instance_id,
      definitionKey: instanceRow.key,
      definitionVersion: instanceRow.version,
      definitionName: instanceRow.name,
      subjectType: instanceRow.subject_type ?? null,
      subjectId: instanceRow.subject_id ?? null,
      status: instanceRow.status,
      outcome: instanceRow.outcome ?? null,
      variables:
        (instanceRow.variables as Record<string, unknown> | null) ?? null,
      startedAt: instanceRow.started_at,
      endedAt: instanceRow.ended_at ?? null,
      currentNodeIds: tokens
        .filter((t) => t.status === 'active')
        .map((t) => t.node_id),
      completedNodeIds: [...milestones.completedNodes],
      activeMilestones: [...milestones.activeNodeIds].map((nodeId) => ({
        nodeId,
        label: views.nodeLabels[nodeId] ?? nodeId,
        responsibility: views.nodeResponsibility[nodeId] ?? null,
      })),
      blockerLabels: [...milestones.blockers],
      nodeLabels: views.nodeLabels,
      nodeDescriptions: views.nodeDescriptions,
      nodeResponsibility: views.nodeResponsibility,
      displayOrder: graph.displayOrder ?? [],
      optionalNodes: optionalNodeIds(graph),
    },
    state: buildStateRollup(tokens, tasks, jobs, commands),
    work: {
      tasks: tasks.map((t) => ({
        id: t.id,
        name: t.name,
        status: t.status,
        nodeId: t.token_id ? nodeIdByToken.get(t.token_id) ?? null : null,
        tokenId: t.token_id ?? null,
        assignee: t.assignee ?? null,
        candidates: t.candidates ?? [],
        dueDate: t.due_date ?? null,
      })),
      jobs: jobViews,
      timers: jobViews.filter((j) => j.type === 'timer'),
      commands: commands.map((c) => ({
        commandId: c.command_id,
        commandType: c.command_type,
        nodeId: c.node_id,
        outcome: c.outcome,
        message: c.message ?? null,
        receiptOutcome:
          receiptRows.find((r) => r.command_id === c.command_id)?.outcome ??
          null,
      })),
      correlations: (correlationRows as Array<{
        workflow_task_id: string
        application_task_id: string | null
        application_task_status: string | null
        application_task_title: string | null
      }>).map((c) => ({
        workflowTaskId: c.workflow_task_id,
        applicationTaskId: c.application_task_id ?? null,
        applicationTaskStatus: c.application_task_status ?? null,
        applicationTaskTitle: c.application_task_title ?? null,
      })),
    },
    history: (eventRows as Array<{
      id: string
      event_type: string
      node_id: string | null
      actor: string | null
      data: Record<string, unknown> | null
      created_at: string
    }>).map((e) => ({
      id: String(e.id),
      eventType: e.event_type,
      nodeId: e.node_id,
      actor: e.actor,
      data: e.data ?? {},
      createdAt: e.created_at,
    })),
    health,
  }
}
