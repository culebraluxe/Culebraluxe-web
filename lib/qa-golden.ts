// ---------------------------------------------------------------------------
// QA-GOLDEN — the deterministic 18-event "Golden QA" narrative for the Flight
// Recorder. It mirrors the original Grok fixture one-to-one in role, ordering,
// systems, timing and causation, but uses REAL current CulebraLuxe QA identities
// (deal / property / people / workflow). Every simulated event is explicitly
// marked qa_simulation: true. Pure and importable so tests can verify it without
// a database.
// ---------------------------------------------------------------------------

export const QA_FIXTURE_VERSION = 2
export const QA_SOURCE_SYSTEM = 'flight-recorder-qa'

/** Deterministic QA evidence ids (never mistaken for real provider artifacts). */
export const QA_IDS = {
  taskSendPns: 'qa-fr-task-send-pns',
  taskNext: 'qa-fr-task-next',
  documentPns: 'qa-fr-document-pns',
  signatureRequest: 'qa-fr-signature-request',
  envelope: 'qa-fr-envelope',
} as const

/**
 * Canonical participant rows for the Golden fixture — conform to migration 034
 * invariants (one ACTIVE structural role per deal) instead of attempting two
 * active `client` rows (which would violate uq_deal_participant_active_structural_role):
 *   - Maria: the canonical client (matches deal.client_person_id semantics).
 *   - Juan:  a secondary buyer via the documented long-tail seam
 *            role='other' + role_label (curated label, never a schema migration),
 *            satisfying uq_deal_participant_active_other_label.
 * Pure and importable so tests can prove the fixture conforms without a database.
 */
export type GoldenParticipantRow = {
  personId: string
  role: 'client' | 'other'
  roleLabel: string | null
  active: true
}

export function goldenParticipantRows(mariaId: string, juanId: string): GoldenParticipantRow[] {
  return [
    { personId: mariaId, role: 'client', roleLabel: null, active: true },
    { personId: juanId, role: 'other', roleLabel: 'co-client', active: true },
  ]
}

/** Real ids the seed binds the narrative to. */
export type QaContext = {
  dealId: string
  propertyId: string
  propertyName: string
  mariaId: string
  juanId: string
  mariaName: string
  juanName: string
  workflowInstanceId: string
  workflowDefinitionKey: string
  workflowDefinitionVersion: number
}

export type GoldenEventSpec = {
  /** 1-based narrative index. */
  index: number
  /** Deterministic sourceEventId, e.g. golden-01-command. */
  sourceEventId: string
  eventType: string
  system: string
  offsetMs: number
  /** sourceEventId of the cause (resolved to the persisted event id at seed time). */
  causeSourceEventId: string | null
  summary: string
  metadata: (ctx: QaContext) => Record<string, unknown>
}

/** Grok-parity timing offsets (ms). */
export const GOLDEN_OFFSETS: number[] = [
  0, 13, 17, 33, 51, 63, 76, 700, 957, 973, 1742, 2367, 2378, 2586, 2597, 2857, 2868, 2870,
]
export function buildGoldenEventSpecs(): GoldenEventSpec[] {
  const spec = (
    index: number,
    eventType: string,
    system: string,
    causeSourceEventId: string | null,
    summary: string,
    metadata: (ctx: QaContext) => Record<string, unknown>,
  ): GoldenEventSpec => ({
    index,
    sourceEventId: `golden-${String(index).padStart(2, '0')}`,
    eventType,
    system,
    offsetMs: GOLDEN_OFFSETS[index - 1],
    causeSourceEventId,
    summary,
    metadata: (ctx) => ({ ...metadata(ctx), qa_simulation: true }),
  })

  return [
    spec(1, 'COMMAND_RECEIVED', 'command', null, 'QA Create Deal command received', (ctx) => ({ command: 'deal.create', dealId: ctx.dealId, initiatedBy: 'flight-recorder-qa' })),
    spec(2, 'DOMAIN_EVENT_EMITTED', 'domain', 'golden-01', 'Deal Aggregate Created', (ctx) => ({ dealId: ctx.dealId, status: 'Draft', version: 1 })),
    spec(3, 'DOMAIN_EVENT_EMITTED', 'domain', 'golden-02', 'Deal Created', (ctx) => ({ dealId: ctx.dealId, property: ctx.propertyName, buyers: [ctx.mariaName, ctx.juanName] })),
    spec(4, 'WORKFLOW_STARTED', 'workflow', 'golden-03', 'Workflow Instance Started', (ctx) => ({ workflowInstanceId: ctx.workflowInstanceId, workflowDefinitionKey: ctx.workflowDefinitionKey, workflowDefinitionVersion: ctx.workflowDefinitionVersion, dealId: ctx.dealId })),
    spec(5, 'TASK_CREATED', 'workflow', 'golden-04', 'Task Created — Send Purchase & Sale to Buyers', (ctx) => ({ taskId: 'qa-fr-task-send-pns', title: 'Send Purchase & Sale to Buyers', assignee: ctx.mariaName, dealId: ctx.dealId, workflowInstanceId: ctx.workflowInstanceId, priority: 'high' })),
    spec(6, 'TASK_ASSIGNED', 'task', 'golden-05', 'Task Assigned', (ctx) => ({ taskId: 'qa-fr-task-send-pns', assignee: ctx.mariaName, dealId: ctx.dealId })),
    spec(7, 'SIGNATURE_ENVELOPE_CREATED', 'signature', 'golden-05', 'BoldSign Envelope Created (simulated)', (ctx) => ({ envelopeId: 'qa-fr-envelope', signatureRequestId: 'qa-fr-signature-request', provider: 'qa-simulation', dealId: ctx.dealId })),
    spec(8, 'SIGNATURE_SENT', 'signature', 'golden-07', 'BoldSign Envelope Sent (simulated)', (ctx) => ({ envelopeId: 'qa-fr-envelope', provider: 'qa-simulation', recipient: ctx.juanName, action: 'sent' })),
    spec(9, 'PERSISTENCE_DEAL_SAVED', 'postgres', 'golden-03', 'Deal Saved', (ctx) => ({ table: 'deal', entityId: ctx.dealId, operation: 'save' })),
    spec(10, 'PERSISTENCE_TASK_SAVED', 'postgres', 'golden-05', 'Task Saved', (ctx) => ({ table: 'task', entityId: 'qa-fr-task-send-pns', operation: 'save' })),
    spec(11, 'SIGNATURE_RECIPIENT_VIEWED', 'signature', 'golden-08', 'BoldSign Recipient Viewed (simulated)', (ctx) => ({ envelopeId: 'qa-fr-envelope', provider: 'qa-simulation', recipient: ctx.juanName, action: 'viewed' })),
    spec(12, 'SIGNATURE_RECIPIENT_SIGNED', 'signature', 'golden-11', 'BoldSign Recipient Signed (simulated)', (ctx) => ({ envelopeId: 'qa-fr-envelope', provider: 'qa-simulation', recipient: ctx.juanName, action: 'signed' })),
    spec(13, 'DOMAIN_EVENT_EMITTED', 'domain', 'golden-12', 'Document Signed', (ctx) => ({ documentId: 'qa-fr-document-pns', signer: ctx.juanName, dealId: ctx.dealId })),
    spec(14, 'TRANSITION_TAKEN', 'workflow', 'golden-13', 'Workflow Advanced', (ctx) => ({ workflowInstanceId: ctx.workflowInstanceId, dealId: ctx.dealId, advanced: true })),
    spec(15, 'PERSISTENCE_ACTIVITY_RECORDED', 'postgres', 'golden-13', 'Activity Recorded', (ctx) => ({ table: 'activity', entityId: ctx.dealId, operation: 'insert' })),
    spec(16, 'TASK_CREATED', 'task', 'golden-14', 'Next Task Created — Schedule Inspections', (ctx) => ({ taskId: 'qa-fr-task-next', title: 'Schedule Inspections', workflowInstanceId: ctx.workflowInstanceId, dealId: ctx.dealId })),
    spec(17, 'PERSISTENCE_TASK_SAVED', 'postgres', 'golden-16', 'Task Saved', (ctx) => ({ table: 'task', entityId: 'qa-fr-task-next', operation: 'save' })),
    spec(18, 'DOMAIN_EVENT_EMITTED', 'domain', 'golden-13', 'Deal Timeline Updated', (ctx) => ({ dealId: ctx.dealId, timeline: 'updated' })),
  ]
}

/** Unique sourceEventIds. */
export function goldenSourceEventIds(): string[] {
  return buildGoldenEventSpecs().map((s) => s.sourceEventId)
}

/** Causal references by sourceEventId (the intended graph). */
export function goldenCauseGraph(): Record<string, string | null> {
  const out: Record<string, string | null> = {}
  for (const s of buildGoldenEventSpecs()) out[s.sourceEventId] = s.causeSourceEventId
  return out
}

