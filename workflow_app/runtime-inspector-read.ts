import { engineConfigured, engineSql } from './engine-client'
import { listTraceEvents } from '../db/workflow-trace'
import { buildRuntimeInspection } from '../lib/runtime-inspector'
import type { RuntimeInspection, TimelineEntry } from '../lib/runtime-inspector'
import type { ProcessGraph } from '../workflow_engine/lib/workflow/types'
import type { QueryExecutor } from '../db/query-executor'

// ---------------------------------------------------------------------------
// RUNTIME-INSPECTOR — read side: definition topology + Flight Recorder overlay.
// ---------------------------------------------------------------------------

export type ResolvedBusinessContext = {
  dealId?: string | null
  propertyId?: string | null
  personId?: string | null
  /** Human-readable labels resolved from the business-context ids (fall back to the id). */
  deal?: string | null
  property?: string | null
  client?: string | null
  workflow?: string | null
}

export type RuntimeInspectionPayload = {
  inspection: RuntimeInspection
  nodeLabels: Record<string, string>
  nodeDescriptions: Record<string, string>
  nodeTypes: Record<string, string>
  businessContext: ResolvedBusinessContext
}

/**
 * Load the design-time ProcessGraph for an instance and overlay the actual
 * Flight Recorder trace evidence. Optionally reconstruct at a past timestamp T
 * (time machine) — visual replay only, never re-execution.
 */
export async function getRuntimeInspection(
  instanceId: string,
  atTimestampIso: string | null = null,
): Promise<RuntimeInspectionPayload | null> {
  if (!engineConfigured()) return null
  const esql = engineSql()
  const graphRows = await esql`
    select
      pd.definition,
      pi.subject_type,
      pi.subject_id
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.id = ${instanceId}
    limit 1
  `
  const row = graphRows[0] as
    | { definition?: ProcessGraph; subject_type?: string | null; subject_id?: string | null }
    | undefined
  const graph: ProcessGraph = row?.definition ?? { startNodeId: '', nodes: {} }
  const subject =
    row && (row.subject_type || row.subject_id)
      ? { subjectType: row.subject_type ?? null, subjectId: row.subject_id ?? null }
      : null

  const events = await listTraceEvents({ workflowInstanceId: instanceId })
  const inspection = buildRuntimeInspection(instanceId, graph, events, atTimestampIso)

  // Resolve the instance's business context to readable labels and backfill the
  // subject-derived ids onto timeline entries that don't carry their own (so
  // every event's detail rows and the Business Context panel show real context).
  const businessContext = await resolveBusinessContext(esql, subject, inspection.timeline)
  const timeline = inspection.timeline.map((e) => ({
    ...e,
    dealId: e.dealId ?? businessContext.dealId ?? null,
    propertyId: e.propertyId ?? businessContext.propertyId ?? null,
    personId: e.personId ?? businessContext.personId ?? null,
  }))
  const enrichedInspection: RuntimeInspection = { ...inspection, timeline }

  const nodeLabels: Record<string, string> = {}
  const nodeDescriptions: Record<string, string> = {}
  const nodeTypes: Record<string, string> = {}
  for (const [id, n] of Object.entries(graph.nodes)) {
    nodeLabels[id] = n.name ?? n.id
    nodeDescriptions[id] = n.description ?? ''
    nodeTypes[id] = n.type ?? ''
  }

  return {
    inspection: enrichedInspection,
    nodeLabels,
    nodeDescriptions,
    nodeTypes,
    businessContext,
  }
}

/** First non-empty TimelineEntry id field. */
function firstId(
  timeline: TimelineEntry[],
  field: 'dealId' | 'propertyId' | 'personId',
): string | null {
  for (const e of timeline) {
    const v = e[field]
    if (v != null && v !== '') return String(v)
  }
  return null
}

// Business-context label lookups. All are observer-reads, parameterized, and
// contained: a failed lookup (unknown id, wrong type, missing table) returns
// null and never breaks the inspector. id::text matches uuid and text pks safely.

async function dealName(esql: QueryExecutor, dealId: string): Promise<string | null> {
  try {
    const rows = await esql`select name as label from deal where id::text = ${dealId} limit 1`
    const row = rows[0] as { label?: string } | undefined
    return row?.label ?? null
  } catch {
    return null
  }
}

async function propertyName(esql: QueryExecutor, propertyId: string): Promise<string | null> {
  try {
    const rows = await esql`select name as label from property where id::text = ${propertyId} limit 1`
    const row = rows[0] as { label?: string } | undefined
    return row?.label ?? null
  } catch {
    return null
  }
}

async function personName(esql: QueryExecutor, personId: string): Promise<string | null> {
  try {
    const rows = await esql`select display_name as label from person where id::text = ${personId} limit 1`
    const row = rows[0] as { label?: string } | undefined
    return row?.label ?? null
  } catch {
    return null
  }
}

async function dealPropertyId(esql: QueryExecutor, dealId: string): Promise<string | null> {
  try {
    const rows = await esql`select property_id::text as pid from deal where id::text = ${dealId} limit 1`
    const row = rows[0] as { pid?: string } | undefined
    return row?.pid ?? null
  } catch {
    return null
  }
}

/**
 * Resolve the workflow's business context: ids come from the instance subject
 * first, then from the first trace row that recorded them; labels come from the
 * deal / property / person tables (a deal-scoped workflow derives its property
 * from the deal). Labels fall back to the id when a row isn't found.
 */
async function resolveBusinessContext(
  esql: QueryExecutor,
  subject: { subjectType?: string | null; subjectId?: string | null } | null,
  timeline: TimelineEntry[],
): Promise<ResolvedBusinessContext> {
  const dealId =
    subject?.subjectType === 'deal' ? (subject.subjectId ?? null) : firstId(timeline, 'dealId')
  let propertyId =
    subject?.subjectType === 'property'
      ? (subject.subjectId ?? null)
      : firstId(timeline, 'propertyId')
  const personId =
    subject?.subjectType === 'person' ? (subject.subjectId ?? null) : firstId(timeline, 'personId')

  if (!propertyId && dealId) propertyId = await dealPropertyId(esql, dealId)

  const [deal, property, client] = await Promise.all([
    dealId ? dealName(esql, dealId) : null,
    propertyId ? propertyName(esql, propertyId) : null,
    personId ? personName(esql, personId) : null,
  ])

  return {
    dealId,
    propertyId,
    personId,
    deal: deal ?? dealId,
    property: property ?? propertyId,
    client: client ?? personId,
  }
}

