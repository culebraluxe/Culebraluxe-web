import { engineConfigured, engineSql } from './engine-client'
import { listTraceEvents } from '../db/workflow-trace'
import { buildRuntimeInspection } from '../lib/runtime-inspector'
import type { RuntimeInspection } from '../lib/runtime-inspector'
import type { ProcessGraph } from '../workflow_engine/lib/workflow/types'

// ---------------------------------------------------------------------------
// RUNTIME-INSPECTOR — read side: definition topology + Flight Recorder overlay.
// ---------------------------------------------------------------------------

export type RuntimeInspectionPayload = {
  inspection: RuntimeInspection
  nodeLabels: Record<string, string>
  nodeDescriptions: Record<string, string>
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
    select pd.definition
    from process_instances pi
    join process_definitions pd on pd.id = pi.definition_id
    where pi.id = ${instanceId}
    limit 1
  `
  const graph: ProcessGraph =
    (graphRows[0] as { definition?: ProcessGraph } | undefined)?.definition ?? {
      startNodeId: '',
      nodes: {},
    }

  const events = await listTraceEvents({ workflowInstanceId: instanceId })
  const inspection = buildRuntimeInspection(instanceId, graph, events, atTimestampIso)

  const nodeLabels: Record<string, string> = {}
  const nodeDescriptions: Record<string, string> = {}
  for (const [id, n] of Object.entries(graph.nodes)) {
    nodeLabels[id] = n.name ?? n.id
    nodeDescriptions[id] = n.description ?? ''
  }

  return { inspection, nodeLabels, nodeDescriptions }
}
