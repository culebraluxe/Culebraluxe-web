import type { QueryExecutor, QueryRow } from './query-executor'
import { neonTx, type TxRunner } from './tx'

// ---------------------------------------------------------------------------
// BoldSign provider store (migration 037, DOC-04).
// Provider-specific persistence remains behind the neutral signature seam.
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

export type CreateBoldSignRequestInput = {
  signatureRequestId: string
  envelopeId: string
  documentIds: string[]
  status: string
}

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
  status: string
  documentIds: string[]
}

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
  retryable: boolean
  /**
   * Raw provider-store status. Normally `error`. A retryable failure during a
   * CREATE call is different: the provider may have accepted the envelope
   * before the response was lost, so callers use `InProgress` to keep the
   * canonical request active/fail-closed until a human resolves the ambiguity.
   */
  status?: string
}

export async function recordBoldSignRequestError(
  input: RecordBoldSignRequestErrorInput,
  run: TxRunner = neonTx,
): Promise<void> {
  const status = input.status ?? 'error'
  await run((tx) => tx`
    insert into bold_sign_request (
      signature_request_id, envelope_id, document_ids, status,
      last_error, error_retryable
    ) values (
      ${input.signatureRequestId}, null, '{}', ${status},
      ${input.error}, ${input.retryable}
    )
    on conflict (signature_request_id)
    do update set status = ${status},
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
