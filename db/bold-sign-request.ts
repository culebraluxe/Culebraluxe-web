import type { QueryExecutor, QueryRow } from './query-executor'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// BoldSign provider store (migration 037, DOC-04).
//
// PROVIDER-SPECIFIC persistence, BEHIND the DOC-03 SignatureProvider seam.
// This table owns EVERYTHING BoldSign-specific: the provider envelope id, the
// provider document/file ids, the last RAW provider status, and the observable
// last_error with a retryable/non-retryable classification. Provider ids and
// provider state NEVER live on the canonical `signature_request` (which stays
// provider-free) and NEVER on `transaction_document` (rejected designs).
//
// Idempotency:
//   - one row per canonical signature_request (PK);
//   - at most one row per BoldSign envelope (partial unique index on
//     envelope_id) — a provider envelope is never persisted twice, so `send`
//     cannot create a duplicate envelope at the provider (the adapter looks
//     the row up first and returns the existing envelope).
//
// The webhook_event table is the webhook replay dedupe (unique
// provider_event_id) AND the durable enqueue record for the DOC-05 async
// reconciler. A replayed webhook inserts nothing (ON CONFLICT DO NOTHING) and
// the adapter returns the same neutral result — the canonical status command
// is itself a no-op on re-application.
//
// These functions never call the provider (rejected design) — the adapter
// (lib/signature/boldsign) composes provider calls with these writes.
// ---------------------------------------------------------------------------

export type BoldSignRequestRow = QueryRow & {
  signature_request_id: string
  envelope_id: string | null
  document_ids: unknown
  status: string
  last_error: string | null
  error_retryable: boolean | null
  created_at: string
  updated_at: string
}

export type BoldSignRequest = {
  signatureRequestId: string
  envelopeId: string | null
  documentIds: string[]
  status: string
  lastError: string | null
  errorRetryable: boolean | null
  createdAt: string
  updatedAt: string
}

export type BoldSignWebhookEvent = {
  id: string
  providerEventId: string
  envelopeId: string
  signatureRequestId: string
  providerEventType: string
  neutralEvent: string
  payload: unknown
  processedAt: string | null
  createdAt: string
}

function mapBoldSignRequest(row: BoldSignRequestRow): BoldSignRequest {
  const documentIds = Array.isArray(row.document_ids)
    ? row.document_ids.map((v) => String(v))
    : []
  return {
    signatureRequestId: row.signature_request_id,
    envelopeId: row.envelope_id ?? null,
    documentIds,
    status: row.status,
    lastError: row.last_error ?? null,
    errorRetryable: row.error_retryable == null ? null : Boolean(row.error_retryable),
    createdAt: row.created_at ? String(row.created_at) : '',
    updatedAt: row.updated_at ? String(row.updated_at) : '',
  }
}

// ---------------------------------------------------------------------------
// Reads (injectable executor; lazy default for production)
// ---------------------------------------------------------------------------

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export async function getBoldSignRequestBySignatureRequestId(
  signatureRequestId: string,
  execute?: QueryExecutor,
): Promise<BoldSignRequest | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select signature_request_id, envelope_id, document_ids, status,
      last_error, error_retryable, created_at, updated_at
    from bold_sign_request
    where signature_request_id = ${signatureRequestId}
    limit 1
  `
  const row = rows[0] as BoldSignRequestRow | undefined
  return row ? mapBoldSignRequest(row) : null
}

export async function getBoldSignRequestByEnvelopeId(
  envelopeId: string,
  execute?: QueryExecutor,
): Promise<BoldSignRequest | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select signature_request_id, envelope_id, document_ids, status,
      last_error, error_retryable, created_at, updated_at
    from bold_sign_request
    where envelope_id = ${envelopeId}
    limit 1
  `
  const row = rows[0] as BoldSignRequestRow | undefined
  return row ? mapBoldSignRequest(row) : null
}

// ---------------------------------------------------------------------------
// Writes (injectable runner; default = the Neon interactive transaction)
// ---------------------------------------------------------------------------

export type CreateBoldSignRequestInput = {
  signatureRequestId: string
  envelopeId: string
  documentIds: string[]
  /** RAW BoldSign status observed after send (mapped to neutral at the seam). */
  status: string
}

/**
 * Persist the provider row for a SUCCESSFUL send. Idempotent at the envelope
 * level: ON CONFLICT (envelope_id) WHERE envelope_id IS NOT NULL DO NOTHING —
 * a duplicate envelope insert is dropped and the winner's row is returned.
 * Returns null only when the conflict target was the PRIMARY KEY
 * (a concurrent duplicate of the SAME request insert won); the caller
 * re-selects by signature_request_id in that case.
 */
export async function createBoldSignRequest(
  input: CreateBoldSignRequestInput,
  run: TxRunner = neonTx,
): Promise<BoldSignRequest | null> {
  const rows = await run((tx) => tx`
    insert into bold_sign_request (
      signature_request_id, envelope_id, document_ids, status
    ) values (
      ${input.signatureRequestId}, ${input.envelopeId},
      ${input.documentIds}, ${input.status}
    )
    on conflict (envelope_id) where envelope_id is not null do nothing
    returning signature_request_id, envelope_id, document_ids, status,
      last_error, error_retryable, created_at, updated_at
  `)
  const row = rows[0] as BoldSignRequestRow | undefined
  return row ? mapBoldSignRequest(row) : null
}

export type UpdateBoldSignRequestStatusInput = {
  signatureRequestId: string
  /** RAW BoldSign status observed (mapped to neutral only at the seam). */
  status: string
  /** Latest observed provider document/file ids. */
  documentIds: string[]
}

/** Cache the last RAW provider status (and document ids) observed by a poll
 *  or cancel. Provider-specific state stays in the provider table. */
export async function updateBoldSignRequestStatus(
  input: UpdateBoldSignRequestStatusInput,
  run: TxRunner = neonTx,
): Promise<void> {
  await run((tx) => tx`
    update bold_sign_request
    set status = ${input.status},
      document_ids = ${input.documentIds},
      updated_at = now()
    where signature_request_id = ${input.signatureRequestId}
  `)
}

export type RecordBoldSignRequestErrorInput = {
  signatureRequestId: string
  error: string
  /** retryable/non-retryable classification of the provider error. */
  retryable: boolean
}

/**
 * Record the observable last_error for a provider failure (send failures have
 * no envelope yet — the row is created with envelope_id NULL; poll/cancel
 * failures update the existing row). The neutral 'error' status is applied to
 * the canonical request by the seam; the DETAIL lives here, provider-specific.
 */
export async function recordBoldSignRequestError(
  input: RecordBoldSignRequestErrorInput,
  run: TxRunner = neonTx,
): Promise<void> {
  await run((tx) => tx`
    insert into bold_sign_request (
      signature_request_id, envelope_id, document_ids, status,
      last_error, error_retryable
    ) values (
      ${input.signatureRequestId}, null, '{}', 'error',
      ${input.error}, ${input.retryable}
    )
    on conflict (signature_request_id)
    do update set status = 'error',
      last_error = ${input.error},
      error_retryable = ${input.retryable},
      updated_at = now()
  `)
}

export type InsertBoldSignWebhookEventInput = {
  providerEventId: string
  envelopeId: string
  signatureRequestId: string
  providerEventType: string
  neutralEvent: string
  payload: unknown
}

/**
 * Enqueue a normalized webhook event. Idempotent by the PROVIDER EVENT ID
 * (the webhook replay dedupe key): a re-delivered webhook inserts nothing and
 * returns false. The row doubles as the durable enqueue record for the DOC-05
 * async reconciler (which sets processed_at once it has handled the event).
 */
export async function insertBoldSignWebhookEvent(
  input: InsertBoldSignWebhookEventInput,
  run: TxRunner = neonTx,
): Promise<boolean> {
  const rows = await run((tx) => tx`
    insert into bold_sign_webhook_event (
      provider_event_id, envelope_id, signature_request_id,
      provider_event_type, neutral_event, payload
    ) values (
      ${input.providerEventId}, ${input.envelopeId}, ${input.signatureRequestId},
      ${input.providerEventType}, ${input.neutralEvent}, ${JSON.stringify(input.payload)}::jsonb
    )
    on conflict (provider_event_id) do nothing
    returning id
  `)
  return rows.length > 0
}

export async function getBoldSignWebhookEventByProviderEventId(
  providerEventId: string,
  execute?: QueryExecutor,
): Promise<BoldSignWebhookEvent | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, provider_event_id, envelope_id, signature_request_id,
      provider_event_type, neutral_event, payload, processed_at, created_at
    from bold_sign_webhook_event
    where provider_event_id = ${providerEventId}
    limit 1
  `
  const row = rows[0]
  if (!row) return null
  return {
    id: String(row.id),
    providerEventId: String(row.provider_event_id),
    envelopeId: String(row.envelope_id),
    signatureRequestId: String(row.signature_request_id),
    providerEventType: String(row.provider_event_type),
    neutralEvent: String(row.neutral_event),
    payload: row.payload,
    processedAt: row.processed_at == null ? null : String(row.processed_at),
    createdAt: row.created_at ? String(row.created_at) : '',
  }
}
