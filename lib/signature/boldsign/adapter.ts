// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration: the BoldSignSignatureProvider adapter.
//
// Implements the DOC-03 SignatureProvider seam (lib/signature/provider.ts)
// and owns EVERY BoldSign-specific concern. The seam and the canonical models
// (signature_request, transaction_document) see only neutral types — BoldSign
// strings/ids never cross this adapter.
//
//   send          — map the neutral send request to a BoldSign template
//                   envelope; persist envelope id + provider document ids in
//                   the provider table (bold_sign_request) keyed by
//                   signature_request_id (one row per request); idempotent —
//                   an existing provider row returns the existing envelope,
//                   never a duplicate. Credentials come from config/env only.
//   status        — poll BoldSign envelope status; cache the RAW status in the
//                   provider table; map to neutral at the seam (the interface
//                   contract: status() returns the status ALREADY mapped).
//   cancel        — best-effort revoke at BoldSign after the seam records
//                   neutral 'voided'; reconciliation converges on divergence.
//   verifyWebhook — verify the BoldSign HMAC signature (X-BoldSign-Signature,
//                   constant-time), normalize the payload to a NEUTRAL event +
//                   the NEUTRAL signature request id, and dedupe by the
//                   provider event id (unique key in bold_sign_webhook_event —
//                   also the DOC-05 async reconciler's durable enqueue).
//
// Provider ids/state live ONLY in bold_sign_request / bold_sign_webhook_event
// (migration 037) — never in transaction_document and never in
// signature_request (rejected designs). Provider errors map to neutral 'error'
// with a retryable/non-retryable classification and an observable last_error.
// ---------------------------------------------------------------------------

import type { SignatureProvider } from '../provider'
import type {
  ProviderActionResult,
  ProviderSendRequest,
  ProviderSendResult,
  ProviderStatusResult,
  SignedArtifactDownload,
  WebhookVerificationResult,
} from '../contracts'
import { BOLD_SIGN_PROVIDER, mapProviderStatus } from '../status-mapping'
import type { QueryExecutor } from '../../../db/query-executor'
import type { TxRunner } from '../../../db/tx'
import {
  createBoldSignRequest,
  getBoldSignRequestByEnvelopeId,
  getBoldSignRequestBySignatureRequestId,
  insertBoldSignWebhookEvent,
  recordBoldSignRequestError,
  updateBoldSignRequestStatus,
} from '../../../db/bold-sign-request'
import type { BoldSignConfig } from './config'
import { BoldSignClient, type BoldSignDirectSigner } from './client'
import { classifyBoldSignError } from './errors'
import { mapBoldSignWebhookEvent } from './events'
import { parseBoldSignWebhookPayload, verifyBoldSignWebhookSignature } from './webhook'

export type BoldSignSignatureProviderDeps = {
  config: BoldSignConfig
  /** Overridable for tests; default: a real client over the config. */
  client?: BoldSignClient
  /** Provider-store executor (tests inject a fake; default: lazy Neon). */
  execute?: QueryExecutor
  /** Provider-store write runner (tests inject a fake; default: Neon tx). */
  run?: TxRunner
  /** Application clock (injectable for deterministic webhook verification). */
  now?: () => Date
}

const INITIAL_ENVELOPE_STATUS = 'InProgress'

// ---------------------------------------------------------------------------
// Direct-PDF source: load the existing unsigned PDF bytes the transaction
// document references (media.file_data) so the adapter can send those exact
// bytes to BoldSign for signing — BoldSign never sees a template.
// ---------------------------------------------------------------------------

let lazyDefaultExecutor: QueryExecutor | null = null

async function lazyExecutor(): Promise<QueryExecutor> {
  if (!lazyDefaultExecutor) {
    const client = await import('../../../db/client')
    lazyDefaultExecutor = client.sql
  }
  return lazyDefaultExecutor
}

type TransactionDocumentPdf = {
  bytes: Uint8Array
  filename: string
  mimeType: string
}

async function loadTransactionDocumentPdf(
  transactionDocumentId: string,
  execute?: QueryExecutor,
): Promise<TransactionDocumentPdf> {
  const q = execute ?? (await lazyExecutor())
  const rows = await q`
    select m.file_data, m.filename, m.mime_type
    from transaction_document d
    join media m on m.id = d.media_id
    where d.id = ${transactionDocumentId}
    limit 1
  `
  const row = rows[0] as
    | { file_data?: unknown; filename?: unknown; mime_type?: unknown }
    | undefined
  if (!row || row.file_data == null) {
    throw new Error(
      `Transaction document ${transactionDocumentId} has no unsigned PDF media to send for signature.`,
    )
  }
  const bytes =
    row.file_data instanceof Uint8Array
      ? new Uint8Array(row.file_data)
      : new Uint8Array(Buffer.from(row.file_data as ArrayLike<number>))
  return {
    bytes,
    filename:
      typeof row.filename === 'string' && row.filename.trim() !== ''
        ? row.filename
        : 'document.pdf',
    mimeType:
      typeof row.mime_type === 'string' && row.mime_type.trim() !== ''
        ? row.mime_type
        : 'application/pdf',
  }
}

export class BoldSignSignatureProvider implements SignatureProvider {
  readonly name = BOLD_SIGN_PROVIDER

  private readonly client: BoldSignClient

  constructor(private readonly deps: BoldSignSignatureProviderDeps) {
    this.client = deps.client ?? new BoldSignClient(deps.config)
  }

  // -------------------------------------------------------------------------
  // send
  // -------------------------------------------------------------------------

  async send(request: ProviderSendRequest): Promise<ProviderSendResult> {
    // Idempotency FIRST: a request that already owns a provider envelope must
    // never create a second one (the seam re-invokes send for a duplicate
    // send; the provider row is the dedupe). The partial unique index on
    // envelope_id (migration 037) is the database backstop.
    const existing = await getBoldSignRequestBySignatureRequestId(
      request.signatureRequestId,
      this.deps.execute,
    )
    if (existing?.envelopeId) {
      return { ok: true, providerStatus: existing.status }
    }

    // Map the neutral recipients onto BoldSign direct-send signers. The neutral
    // role/order vocabulary stays neutral — this mapping is adapter-internal.
    const signers: BoldSignDirectSigner[] = request.recipients.map((recipient) => ({
      name: recipient.name,
      emailAddress: recipient.email,
      signerType: (recipient.role === 'approver' ? 'Reviewer' : 'Signer') as
        | 'Signer'
        | 'Reviewer',
      roleIndex: recipient.order,
      order: recipient.order,
    }))

    try {
      // Send the EXISTING unsigned PDF bytes CulebraLuxe already owns directly
      // to BoldSign (multipart POST /v1/document/send) — no template needed.
      const pdf = await loadTransactionDocumentPdf(
        request.transactionDocumentId,
        this.deps.execute,
      )
      const created = await this.client.sendDocument({
        fileBytes: pdf.bytes,
        filename: pdf.filename,
        mimeType: pdf.mimeType,
        title: 'Signature request',
        message: request.message,
        signers,
        enableSigningOrder: signers.length > 1,
      })
      let row = await createBoldSignRequest(
        {
          signatureRequestId: request.signatureRequestId,
          envelopeId: created.documentId,
          // Provider file ids are observed on the first status poll.
          documentIds: [],
          status: INITIAL_ENVELOPE_STATUS,
        },
        this.deps.run,
      )
      if (!row) {
        // A concurrent duplicate of the SAME request won the insert (PK
        // conflict): return the winner's row — never a second envelope.
        row = await getBoldSignRequestBySignatureRequestId(
          request.signatureRequestId,
          this.deps.execute,
        )
      }
      if (!row?.envelopeId) {
        const message =
          row?.lastError ??
          'BoldSign send did not produce a provider envelope (concurrent failure).'
        return { ok: false, providerStatus: 'error', error: message }
      }
      return { ok: true, providerStatus: row.status }
    } catch (err) {
      const classified = classifyBoldSignError(err)
      // Observable last_error with the retryable classification — provider
      // detail only; the seam maps the failure to neutral 'error' separately.
      await recordBoldSignRequestError(
        {
          signatureRequestId: request.signatureRequestId,
          error: classified.message,
          retryable: classified.retryable,
        },
        this.deps.run,
      )
      return { ok: false, providerStatus: 'error', error: classified.message }
    }
  }

  // -------------------------------------------------------------------------
  // status
  // -------------------------------------------------------------------------

  async status(requestId: string): Promise<ProviderStatusResult> {
    const row = await getBoldSignRequestBySignatureRequestId(requestId, this.deps.execute)
    if (!row?.envelopeId) {
      // Unknown request (or a send that never produced an envelope).
      return { status: 'error' }
    }
    try {
      const props = await this.client.getDocumentProperties(row.envelopeId)
      const documentIds =
        row.documentIds.length > 0 ? row.documentIds : props.fileIds
      await updateBoldSignRequestStatus(
        {
          signatureRequestId: requestId,
          status: props.status,
          documentIds,
        },
        this.deps.run,
      )
      // Mapped to neutral AT THE SEAM boundary — the caller never sees a
      // BoldSign status string.
      return { status: mapProviderStatus(this.name, props.status) }
    } catch (err) {
      const classified = classifyBoldSignError(err)
      await recordBoldSignRequestError(
        { signatureRequestId: requestId, error: classified.message, retryable: classified.retryable },
        this.deps.run,
      )
      return { status: 'error' }
    }
  }

  // -------------------------------------------------------------------------
  // cancel
  // -------------------------------------------------------------------------

  async cancel(requestId: string): Promise<ProviderActionResult> {
    const row = await getBoldSignRequestBySignatureRequestId(requestId, this.deps.execute)
    if (!row?.envelopeId) {
      return { ok: false, error: 'No BoldSign envelope exists for this request.' }
    }
    try {
      await this.client.revokeDocument(row.envelopeId)
      await updateBoldSignRequestStatus(
        { signatureRequestId: requestId, status: 'Revoked', documentIds: row.documentIds },
        this.deps.run,
      )
      return { ok: true }
    } catch (err) {
      const classified = classifyBoldSignError(err)
      return { ok: false, error: classified.message }
    }
  }

  // -------------------------------------------------------------------------
  // DOC-05 — one-time signed-artifact download (via the provider table)
  // -------------------------------------------------------------------------

  async downloadSignedArtifact(requestId: string): Promise<SignedArtifactDownload> {
    const row = await getBoldSignRequestBySignatureRequestId(requestId, this.deps.execute)
    if (!row?.envelopeId) {
      throw new Error(
        `BoldSign: no envelope exists for signature request ${requestId}; the signed artifact cannot be downloaded.`,
      )
    }
    // The adapter resolves its own provider envelope id through its provider
    // table (DOC-04); no provider id crosses the seam. The download is
    // read-only — provider state is never written here.
    return this.client.downloadDocument(row.envelopeId)
  }

  // -------------------------------------------------------------------------
  // webhook
  // -------------------------------------------------------------------------

  async verifyWebhook(
    payload: unknown,
    signature: string,
  ): Promise<WebhookVerificationResult> {
    // HMAC is verified over the RAW body bytes (BoldSign signs
    // `${t}.${rawBody}`); a parsed object can never reproduce those bytes.
    if (typeof payload !== 'string') {
      throw new Error(
        'BoldSign webhook payload must be the raw request body string (HMAC is verified over the exact raw bytes).',
      )
    }
    const nowSeconds = Math.floor((this.deps.now?.() ?? new Date()).getTime() / 1000)
    verifyBoldSignWebhookSignature(
      payload,
      signature,
      this.deps.config.webhookSecret,
      nowSeconds,
      this.deps.config.webhookToleranceSeconds,
    )

    const normalized = parseBoldSignWebhookPayload(payload)
    const row = await getBoldSignRequestByEnvelopeId(normalized.envelopeId, this.deps.execute)
    if (!row) {
      throw new Error(`BoldSign webhook for unknown envelope ${normalized.envelopeId}.`)
    }
    // Normalize the BoldSign event onto the neutral provider event vocabulary
    // (fail closed when it maps to nothing the seam can apply).
    const neutral = mapBoldSignWebhookEvent(normalized.eventType, normalized.documentStatus)

    // Dedupe by the PROVIDER EVENT ID (unique key): a replayed webhook inserts
    // nothing and returns the same neutral result — the seam's canonical
    // status command is itself a no-op on re-application. The row is also the
    // durable enqueue record for the DOC-05 async reconciler.
    await insertBoldSignWebhookEvent(
      {
        providerEventId: normalized.providerEventId,
        envelopeId: normalized.envelopeId,
        signatureRequestId: row.signatureRequestId,
        providerEventType: normalized.eventType,
        neutralEvent: neutral,
        payload: JSON.parse(payload),
      },
      this.deps.run,
    )
    return { event: neutral, signatureRequestId: row.signatureRequestId }
  }
}
