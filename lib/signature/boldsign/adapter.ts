// ---------------------------------------------------------------------------
// DOC-04 — BoldSign provider adapter. BoldSign ids/state stay behind the
// provider-neutral SignatureProvider seam.
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
import {
  parseFormSignatureAnchors,
  resolveFormSignatureAnchors,
  type FormSignatureAnchor,
} from '../../forms/signature-anchors'

export type BoldSignSignatureProviderDeps = {
  config: BoldSignConfig
  client?: BoldSignClient
  execute?: QueryExecutor
  run?: TxRunner
  now?: () => Date
}

const INITIAL_ENVELOPE_STATUS = 'InProgress'

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
  signatureAnchors: FormSignatureAnchor[]
}

export function pdfAnchorToBoldSignBounds(anchor: FormSignatureAnchor) {
  return {
    x: anchor.rect.x,
    y: anchor.pageHeight - anchor.rect.y - anchor.rect.height,
    width: anchor.rect.width,
    height: anchor.rect.height,
  }
}

function anchorFields(anchors: readonly FormSignatureAnchor[]) {
  const order = { signature: 0, initial: 1, date: 2 } as const
  return [...anchors]
    .sort((a, b) => order[a.kind] - order[b.kind])
    .map((anchor) => ({
      fieldType:
        anchor.kind === 'signature'
          ? 'Signature'
          : anchor.kind === 'initial'
            ? 'Initial'
            : 'DateSigned',
      pageNumber: anchor.pageIndex + 1,
      bounds: pdfAnchorToBoldSignBounds(anchor),
      isRequired: true,
      fontSize:
        anchor.kind === 'signature' ? 14 : anchor.kind === 'initial' ? 10 : 9,
      ...(anchor.kind === 'date' ? { dateFormat: 'MMM dd, yyyy' } : {}),
    }))
}

function groupAnchorSets(anchors: readonly FormSignatureAnchor[]) {
  const grouped = new Map<string, FormSignatureAnchor[]>()
  for (const anchor of anchors) {
    const key = `${anchor.role}:${anchor.slotId ?? ''}`
    const current = grouped.get(key) ?? []
    current.push(anchor)
    grouped.set(key, current)
  }
  return grouped
}

async function loadTransactionDocumentPdf(
  transactionDocumentId: string,
  execute?: QueryExecutor,
): Promise<TransactionDocumentPdf> {
  const q = execute ?? (await lazyExecutor())
  const rows = await q`
    select m.file_data, m.filename, m.mime_type, d.source_snapshot
    from transaction_document d
    join media m on m.id = d.media_id
    where d.id = ${transactionDocumentId}
    limit 1
  `
  const row = rows[0] as
    | {
        file_data?: unknown
        filename?: unknown
        mime_type?: unknown
        source_snapshot?: unknown
      }
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
  let sourceSnapshot: Record<string, unknown> | null = null
  if (row.source_snapshot && typeof row.source_snapshot === 'object') {
    sourceSnapshot = row.source_snapshot as Record<string, unknown>
  } else if (typeof row.source_snapshot === 'string') {
    try {
      const parsed = JSON.parse(row.source_snapshot) as unknown
      if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
        sourceSnapshot = parsed as Record<string, unknown>
      }
    } catch {
      sourceSnapshot = null
    }
  }
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
    signatureAnchors: parseFormSignatureAnchors(sourceSnapshot?.signatureAnchors),
  }
}

export class BoldSignSignatureProvider implements SignatureProvider {
  readonly name = BOLD_SIGN_PROVIDER
  private readonly client: BoldSignClient

  constructor(private readonly deps: BoldSignSignatureProviderDeps) {
    this.client = deps.client ?? new BoldSignClient(deps.config)
  }

  async send(request: ProviderSendRequest): Promise<ProviderSendResult> {
    const existing = await getBoldSignRequestBySignatureRequestId(
      request.signatureRequestId,
      this.deps.execute,
    )
    if (existing?.envelopeId) {
      return { ok: true, providerStatus: existing.status }
    }
    if (existing?.errorRetryable) {
      // A previous CREATE attempt ended ambiguously (timeout/network/5xx).
      // We do not know whether BoldSign accepted the envelope. Never issue a
      // second legal envelope automatically; keep the canonical request active
      // until the provider account is reconciled by a human/operator path.
      return {
        ok: false,
        providerStatus: INITIAL_ENVELOPE_STATUS,
        error:
          existing.lastError ??
          'BoldSign send outcome is uncertain; do not resend until provider state is resolved.',
      }
    }

    try {
      const pdf = await loadTransactionDocumentPdf(
        request.transactionDocumentId,
        this.deps.execute,
      )
      if (pdf.signatureAnchors.length === 0) {
        throw new Error(
          'The issued PDF has no immutable signature anchors. Reissue the document with the current Forms renderer before sending it for signature.',
        )
      }
      const availableSets = groupAnchorSets(pdf.signatureAnchors)
      const orderedSets = [...availableSets.values()]
      const perRecipient = request.recipients.map((recipient, index) => {
        const role = recipient.executionRole ?? request.signatureRole ?? null
        const slotId = recipient.executionSlotId ?? request.signatureSlotId ?? null
        if (role && slotId) return availableSets.get(`${role}:${slotId}`) ?? []
        if (!role && !slotId && orderedSets.length === request.recipients.length) {
          return orderedSets[index]
        }
        return resolveFormSignatureAnchors(pdf.signatureAnchors, { role, slotId })
      })
      if (
        perRecipient.length !== request.recipients.length ||
        perRecipient.some((anchors) => anchors.length === 0)
      ) {
        throw new Error(
          'The issued PDF signature anchor is missing or ambiguous for the selected participant. Reissue the document and select a declared signature role.',
        )
      }
      const signers: BoldSignDirectSigner[] = request.recipients.map(
        (recipient, index) => ({
          name: recipient.name,
          emailAddress: recipient.email,
          signerType: (recipient.role === 'approver' ? 'Reviewer' : 'Signer') as
            | 'Signer'
            | 'Reviewer',
          signerOrder: recipient.order,
          authenticationType: 'EmailOTP',
          formFields: anchorFields(perRecipient[index]),
        }),
      )
      const created = await this.client.sendDocument({
        fileBytes: pdf.bytes,
        filename: pdf.filename,
        mimeType: pdf.mimeType,
        title: 'Signature request',
        message: request.message,
        signers,
        enableSigningOrder: signers.length > 1,
        completionCcEmails: request.completionRecipientEmails ?? [],
      })
      let row = await createBoldSignRequest(
        {
          signatureRequestId: request.signatureRequestId,
          envelopeId: created.documentId,
          documentIds: [],
          status: INITIAL_ENVELOPE_STATUS,
        },
        this.deps.run,
      )
      if (!row) {
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
      await recordBoldSignRequestError(
        {
          signatureRequestId: request.signatureRequestId,
          error: classified.message,
          retryable: classified.retryable,
          // Retryable CREATE failures have an UNKNOWN provider outcome. Keep
          // them active instead of freeing the canonical slot for a resend.
          status: classified.retryable ? INITIAL_ENVELOPE_STATUS : 'error',
        },
        this.deps.run,
      )
      return {
        ok: false,
        providerStatus: classified.retryable ? INITIAL_ENVELOPE_STATUS : 'error',
        error: classified.message,
      }
    }
  }

  async status(requestId: string): Promise<ProviderStatusResult> {
    const row = await getBoldSignRequestBySignatureRequestId(requestId, this.deps.execute)
    if (!row?.envelopeId) {
      // A retryable create failure is an UNKNOWN provider outcome, not proof
      // that no envelope exists. Preserve the active slot; only a definite
      // non-retryable send failure is terminal error.
      return { status: row?.errorRetryable ? 'sent' : 'error' }
    }
    try {
      const props = await this.client.getDocumentProperties(row.envelopeId)
      const documentIds = row.documentIds.length > 0 ? row.documentIds : props.fileIds
      await updateBoldSignRequestStatus(
        { signatureRequestId: requestId, status: props.status, documentIds },
        this.deps.run,
      )
      return { status: mapProviderStatus(this.name, props.status) }
    } catch (err) {
      const classified = classifyBoldSignError(err)
      await recordBoldSignRequestError(
        {
          signatureRequestId: requestId,
          error: classified.message,
          retryable: classified.retryable,
          // A transient poll failure does not make an existing envelope
          // terminal. Preserve its last known provider status.
          status: classified.retryable ? row.status : 'error',
        },
        this.deps.run,
      )
      return {
        status: classified.retryable
          ? mapProviderStatus(this.name, row.status)
          : 'error',
      }
    }
  }

  async cancel(requestId: string): Promise<ProviderActionResult> {
    const row = await getBoldSignRequestBySignatureRequestId(requestId, this.deps.execute)
    if (!row?.envelopeId) {
      return {
        ok: false,
        error: row?.errorRetryable
          ? 'BoldSign send outcome is uncertain; provider envelope id is unknown and automatic cancellation is unsafe.'
          : 'No BoldSign envelope exists for this request.',
      }
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
      await recordBoldSignRequestError(
        {
          signatureRequestId: requestId,
          error: classified.message,
          retryable: classified.retryable,
          status: row.status,
        },
        this.deps.run,
      )
      return { ok: false, error: classified.message }
    }
  }

  async downloadSignedArtifact(requestId: string): Promise<SignedArtifactDownload> {
    const row = await getBoldSignRequestBySignatureRequestId(requestId, this.deps.execute)
    if (!row?.envelopeId) {
      throw new Error(
        `BoldSign: no envelope exists for signature request ${requestId}; the signed artifact cannot be downloaded.`,
      )
    }
    return this.client.downloadDocument(row.envelopeId)
  }

  async downloadAuditTrail(requestId: string): Promise<SignedArtifactDownload> {
    const row = await getBoldSignRequestBySignatureRequestId(requestId, this.deps.execute)
    if (!row?.envelopeId) {
      throw new Error(
        `BoldSign: no envelope exists for signature request ${requestId}; the audit trail cannot be downloaded.`,
      )
    }
    return this.client.downloadAuditTrail(row.envelopeId)
  }

  async verifyWebhook(
    payload: unknown,
    signature: string,
  ): Promise<WebhookVerificationResult> {
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
    const neutral = mapBoldSignWebhookEvent(normalized.eventType, normalized.documentStatus)
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
