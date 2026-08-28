// ---------------------------------------------------------------------------
// WORKFLOW-TRACE — Flight Recorder vocabulary + bounded-detail contract.
//
// A durable, immutable, observer-only execution trace. These are the small,
// extensible event types and the bounded metadata contract that the recorder
// honors. No application behavior may depend on recorder success: recording is
// operational evidence, never a business transaction gate.
// ---------------------------------------------------------------------------

/**
 * Controlled, extensible trace event vocabulary. Records architecturally
 * meaningful boundaries, not every line of application execution. New types are
 * additive strings (extensible rather than a giant permanent taxonomy).
 */
export type TraceEventType =
  // COMMAND
  | 'COMMAND_RECEIVED'
  | 'COMMAND_REPLAYED'
  | 'COMMAND_COMPLETED'
  | 'COMMAND_FAILED'
  // DOMAIN
  | 'DOMAIN_EVENT_EMITTED'
  // WORKFLOW
  | 'WORKFLOW_STARTED'
  | 'NODE_ENTERED'
  | 'NODE_COMPLETED'
  | 'TRANSITION_TAKEN'
  | 'WORKFLOW_COMPLETED'
  | 'WORKFLOW_FAILED'
  // TASK
  | 'TASK_CREATED'
  | 'TASK_ASSIGNED'
  | 'TASK_COMPLETED'
  // TIMER / JOB
  | 'TIMER_SCHEDULED'
  | 'TIMER_FIRED'
  | 'TIMER_CANCELLED'
  | 'JOB_STARTED'
  | 'JOB_COMPLETED'
  | 'JOB_FAILED'
  // DOCUMENT
  | 'DOCUMENT_CREATED'
  | 'DOCUMENT_ISSUED'
  | 'DOCUMENT_EXECUTED'
  // SIGNATURE
  | 'SIGNATURE_REQUEST_CREATED'
  | 'SIGNATURE_SENT'
  | 'SIGNER_COMPLETED'
  | 'SIGNATURE_COMPLETED'
  | 'SIGNATURE_FAILED'
  // ERROR / RECOVERY
  | 'FAILURE'
  | 'RETRY'
  | 'RECOVERED'
  // Extensible
  | (string & {})

export type TraceOutcome =
  | 'SUCCESS'
  | 'FAILURE'
  | 'STARTED'
  | 'RECOVERED'
  | 'REPLAYED'
  | (string & {})

export type TraceEvent = {
  eventType: TraceEventType
  system: string
  occurredAt: string
  completedAt: string | null
  durationMs: number | null
  outcome: TraceOutcome | null

  traceId: string | null
  correlationId: string | null
  causationId: string | null

  dealId: string | null
  personId: string | null
  propertyId: string | null
  transactionDocumentId: string | null

  workflowInstanceId: string | null
  workflowDefinitionKey: string | null
  workflowDefinitionVersion: number | null
  workflowNodeId: string | null
  workflowTransitionId: string | null

  commandId: string | null
  domainEventId: string | null
  taskId: string | null
  timerJobId: string | null
  signatureRequestId: string | null
  externalReference: string | null

  summary: string | null
  metadata: Record<string, unknown> | null

  sourceSystem: string | null
  sourceEventId: string | null
}

/** Keys never persisted even when a producer passes them (defense in depth). */
const SENSITIVE_KEY_RE =
  /(password|passwd|secret|token|credential|authorization|auth[_-]?header|api[_-]?key|database[_-]?url|db[_-]?url|dsn|connection[_-]?string|private[_-]?key|access[_-]?token|refresh[_-]?token|client[_-]?secret|signature[_-]?secret)/i

const MAX_DEPTH = 6
const MAX_STRING_LENGTH = 500

function sanitizeValue(value: unknown, depth: number, key: string): unknown {
  if (SENSITIVE_KEY_RE.test(key)) return '[redacted]'
  if (value == null) return null
  if (typeof value === 'string') {
    if (value.length > MAX_STRING_LENGTH) return value.slice(0, MAX_STRING_LENGTH) + '…'
    return value
  }
  if (typeof value === 'number' || typeof value === 'boolean') return value
  if (Array.isArray(value)) {
    if (depth >= MAX_DEPTH) return '[array]'
    return value.map((v) => sanitizeValue(v, depth + 1, key))
  }
  if (typeof value === 'object') {
    if (depth >= MAX_DEPTH) return '[object]'
    const out: Record<string, unknown> = {}
    for (const [k, v] of Object.entries(value as Record<string, unknown>)) {
      out[k] = sanitizeValue(v, depth + 1, k)
    }
    return out
  }
  return String(value)
}

/**
 * Bounded, secret-safe metadata. Strips credentials/tokens/URLs/keys, caps
 * nested depth and string length, so the trace never persists secrets or
 * unbounded payload dumps. Returns null for empty input.
 */
export function sanitizeMetadata(
  input: Record<string, unknown> | null | undefined,
): Record<string, unknown> | null {
  if (input == null || Object.keys(input).length === 0) return null
  const out: Record<string, unknown> = {}
  for (const [k, v] of Object.entries(input)) {
    out[k] = sanitizeValue(v, 0, k)
  }
  return out
}
