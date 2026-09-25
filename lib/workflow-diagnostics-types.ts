export type WorkflowDefinitionSummary = {
  definitionId: string
  key: string
  version: number
  name: string
  status: string
  instanceCount: number
  activeCount: number
}

export type WorkflowInstanceRow = {
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
  propertyName?: string | null
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

export type WorkflowAnomaly = {
  kind: string
  severity: string
  instanceId: string | null
  subjectId: string | null
  message: string
}

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

export type WorkflowDiagnosticsSnapshot = {
  configured: boolean
  summary: WorkflowDiagnosticsSummary
  definitions: WorkflowDefinitionSummary[]
  instances: WorkflowInstanceRow[]
  anomalies: WorkflowAnomaly[]
}

export type InstanceDetail = WorkflowInstanceRow & {
  variables: Record<string, unknown> | null
  nodeLabels: Record<string, string>
  tokens: TokenRow[]
  tasks: TaskRow[]
  jobs: Array<{ id: string; type: string; status: string; dueAt: string | null }>
  events: Array<{ id: string; eventType: string; nodeId: string | null; actor: string | null }>
  correlations: CorrelationRow[]
  commands: CommandRow[]
}
