import type { NodeRuntime } from '@/lib/runtime-inspector'
import type { ProcessGraph } from '@/workflow_engine/lib/workflow/types'

export type FlightRecorderMappedNode = {
  id: string
  name: string | null
  type: string | null
  description: string | null
}

export type FlightRecorderEvent = {
  eventId: string
  occurredAt: string
  eventType: string
  sourceSystem: string
  summary: string | null
  outcome: string | null
  durationMs: number | null
  traceId: string | null
  correlationId: string | null
  workflowInstanceId: string | null
  workflowNodeId: string | null
  causationId: string | null
  commandId: string | null
  domainEventId: string | null
  documentId: string | null
  signatureRequestId: string | null
  metadata: Record<string, unknown> | null
  mappedWorkflowNode: FlightRecorderMappedNode | null
}

export type FlightRecorderWorkflow = {
  workflowInstanceId: string
  definitionId: string | null
  definitionKey: string | null
  definitionVersion: number | null
  definitionMissing: boolean
  status: string | null
  currentNodeId: string | null
  graph: ProcessGraph
  nodeStates: Record<string, NodeRuntime>
}

export type FlightRecorderTransaction = {
  transaction: {
    dealId: string | null
    property: string | null
    client: string | null
    correlationId: string | null
    status: string | null
    initiatedBy: string | null
    initiatedAt: string | null
  }
  workflows: FlightRecorderWorkflow[]
  events: FlightRecorderEvent[]
  instances: { shown: number; total: number } | null
}
