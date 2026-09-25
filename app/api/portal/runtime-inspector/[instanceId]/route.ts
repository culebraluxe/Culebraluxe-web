import { NextRequest, NextResponse } from 'next/server'

import {
  buildRuntimeInspection,
  type ProcessGraph,
} from '@/lib/runtime-inspector'
import { rustApiRead } from '@/lib/rust-api/client'
import type { TraceEvent } from '@/lib/workflow-trace'
import { withApiHandler } from '@/lib/error-capture-seam'

export const dynamic = 'force-dynamic'

type RustFlightWorkflow = {
  workflowInstanceId: string
  definitionKey: string | null
  definitionVersion: number | null
  graph: unknown
}

type RustFlightEvent = {
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
}

type RustFlightRecorder = {
  transaction: {
    dealId: string | null
    property: string | null
    client: string | null
  }
  workflows: RustFlightWorkflow[]
  events: RustFlightEvent[]
}

function metadataString(
  metadata: Record<string, unknown> | null,
  ...keys: string[]
): string | null {
  if (!metadata) return null
  for (const key of keys) {
    const value = metadata[key]
    if (typeof value === 'string' && value.trim()) return value
  }
  return null
}

function toTraceEvent(
  event: RustFlightEvent,
  workflow: RustFlightWorkflow,
): TraceEvent {
  return {
    id: event.eventId,
    eventType: event.eventType,
    system: event.sourceSystem,
    occurredAt: event.occurredAt,
    completedAt: null,
    durationMs: event.durationMs,
    outcome: event.outcome,
    traceId: event.traceId,
    correlationId: event.correlationId,
    causationId: event.causationId,
    dealId: metadataString(event.metadata, 'dealId', 'deal_id'),
    personId: metadataString(event.metadata, 'personId', 'person_id'),
    propertyId: metadataString(event.metadata, 'propertyId', 'property_id'),
    transactionDocumentId:
      event.documentId ??
      metadataString(event.metadata, 'transactionDocumentId', 'transaction_document_id'),
    workflowInstanceId: event.workflowInstanceId,
    workflowDefinitionKey: workflow.definitionKey,
    workflowDefinitionVersion: workflow.definitionVersion,
    workflowNodeId: event.workflowNodeId,
    workflowTransitionId: metadataString(
      event.metadata,
      'workflowTransitionId',
      'workflow_transition_id',
      'transitionId',
    ),
    commandId: event.commandId,
    domainEventId: event.domainEventId,
    taskId: metadataString(event.metadata, 'taskId', 'task_id'),
    timerJobId: metadataString(event.metadata, 'timerJobId', 'timer_job_id'),
    signatureRequestId: event.signatureRequestId,
    externalReference: metadataString(
      event.metadata,
      'externalReference',
      'external_reference',
    ),
    summary: event.summary,
    metadata: event.metadata,
    sourceSystem: event.sourceSystem,
    sourceEventId: event.eventId,
  }
}

async function GETHandler(
  req: NextRequest,
  { params }: { params: Promise<{ instanceId: string }> },
) {
  const { instanceId } = await params
  const at = req.nextUrl.searchParams.get('at')
  const atIso = at && at !== 'now' ? at : null

  try {
    const result = await rustApiRead<RustFlightRecorder>(
      (`/v1/flight-recorder/${encodeURIComponent(instanceId)}`) as `/v1/${string}`,
    )
    const snapshot = result.value
    const workflow =
      snapshot.workflows.find(
        (candidate) => candidate.workflowInstanceId === instanceId,
      ) ?? snapshot.workflows[0]

    if (!workflow) {
      return NextResponse.json({ error: 'not_found' }, { status: 404 })
    }

    const graph = workflow.graph as ProcessGraph
    const events = snapshot.events
      .filter((event) => event.workflowInstanceId === instanceId)
      .map((event) => toTraceEvent(event, workflow))

    const inspection = buildRuntimeInspection(
      instanceId,
      graph,
      events,
      atIso,
    )

    const nodeLabels: Record<string, string> = {}
    const nodeDescriptions: Record<string, string> = {}
    const nodeTypes: Record<string, string> = {}
    for (const [id, node] of Object.entries(graph.nodes ?? {})) {
      nodeLabels[id] = node.name ?? node.id ?? id
      nodeDescriptions[id] = node.description ?? ''
      nodeTypes[id] = node.type ?? ''
    }

    const first = inspection.timeline[0]
    const dealId =
      snapshot.transaction.dealId ??
      inspection.timeline.find((entry) => entry.dealId)?.dealId ??
      null
    const propertyId =
      inspection.timeline.find((entry) => entry.propertyId)?.propertyId ?? null
    const personId =
      inspection.timeline.find((entry) => entry.personId)?.personId ?? null

    // Backfill the business IDs already proved by the Rust trace/transaction
    // onto timeline rows that do not carry their own narrower context.
    inspection.timeline = inspection.timeline.map((entry) => ({
      ...entry,
      dealId: entry.dealId ?? dealId,
      propertyId: entry.propertyId ?? propertyId,
      personId: entry.personId ?? personId,
    }))

    return NextResponse.json({
      inspection,
      nodeLabels,
      nodeDescriptions,
      nodeTypes,
      businessContext: {
        dealId,
        propertyId,
        personId,
        deal: dealId,
        property: snapshot.transaction.property ?? propertyId,
        client: snapshot.transaction.client ?? personId,
        workflow: workflow.definitionKey ?? first?.workflowDefinitionKey ?? null,
      },
    })
  } catch (error) {
    console.error(
      '[runtime-inspector] Rust read failed:',
      error instanceof Error ? error.message : error,
    )
    return NextResponse.json(
      { error: 'runtime_inspector_unavailable' },
      { status: 503 },
    )
  }
}

export const GET = withApiHandler(
  {
    label: '/api/portal/runtime-inspector/[instanceId]',
    route: '/api/portal/runtime-inspector/[instanceId]',
  },
  GETHandler,
)
