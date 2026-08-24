// ---------------------------------------------------------------------------
// DOC-03 — Signature Provider Seam: provider-neutral contracts.
//
// This module is the compile-ready contract surface of the provider-neutral
// signing boundary. It is shaped like the existing workflow command seam
// (lib/workflow/contracts.ts): pure types + constants, no runtime behavior
// beyond the status/transition model, no provider imports, no SDK objects.
//
// Boundaries (architect brief):
//   - Canonical domain models stay provider-free. The neutral status model
//     below is stored on the canonical `signature_request` record (migration
//     036), which references transaction_document_id and NEVER carries
//     provider-specific ids/state — those live in a DOC-04 provider table
//     behind the SignatureProvider seam.
//   - The seam maps provider status -> neutral status
//     (lib/signature/status-mapping.ts). transaction_document reflects only
//     the FINAL signed outcome via DOC-05 reconciliation — never intermediate
//     provider state.
//   - Commands are neutral: signature.request.send / .status / .cancel /
//     .decline. All are idempotent via claim-first command receipts
//     (db/workflow-command-receipt.ts); a send for the same
//     transaction_document + an active request returns the existing request,
//     never a duplicate.
//   - Provider adapters (DOC-04 BoldSign) implement SignatureProvider
//     (lib/signature/provider.ts); the application router
//     (lib/signature/application.ts) dispatches by the configured provider,
//     never by provider-specific command.
// ---------------------------------------------------------------------------

import type { DomainEventType } from '../workflow/contracts'
import {
  SIGNATURE_REQUEST_CANCEL,
  SIGNATURE_REQUEST_DECLINE,
  SIGNATURE_REQUEST_SEND,
  SIGNATURE_REQUEST_STATUS,
} from '../commands/command-types'

// Re-export the stable command identifiers (single source of truth:
// lib/commands/command-types.ts) so the seam is self-contained.
export {
  SIGNATURE_REQUEST_CANCEL,
  SIGNATURE_REQUEST_DECLINE,
  SIGNATURE_REQUEST_SEND,
  SIGNATURE_REQUEST_STATUS,
}

// ---------------------------------------------------------------------------
// Neutral status model
// ---------------------------------------------------------------------------

export const SIGNATURE_REQUEST_STATUSES = [
  'requested',
  'sent',
  'viewed',
  'signed',
  'completed',
  'declined',
  'voided',
  'expired',
  'error',
] as const

export type SignatureRequestStatus = (typeof SIGNATURE_REQUEST_STATUSES)[number]

/** Non-terminal statuses: a request in one of these owns the active slot. */
export const SIGNATURE_REQUEST_ACTIVE_STATUSES = [
  'requested',
  'sent',
  'viewed',
  'signed',
] as const

export type SignatureRequestActiveStatus =
  (typeof SIGNATURE_REQUEST_ACTIVE_STATUSES)[number]

/** Terminal statuses: no further transitions; a new send is a new request. */
export const SIGNATURE_REQUEST_TERMINAL_STATUSES = [
  'completed',
  'declined',
  'voided',
  'expired',
  'error',
] as const

export type SignatureRequestTerminalStatus =
  (typeof SIGNATURE_REQUEST_TERMINAL_STATUSES)[number]

/**
 * Legal neutral status transitions (the story's model:
 * requested -> sent -> viewed -> signed -> completed, plus declined/voided/
 * expired/error sinks). Provider observations and application commands
 * (cancel -> voided, decline -> declined) are applied only along these edges.
 */
export const SIGNATURE_REQUEST_TRANSITIONS: Record<
  SignatureRequestStatus,
  readonly SignatureRequestStatus[]
> = {
  requested: ['sent', 'viewed', 'declined', 'voided', 'expired', 'error'],
  sent: ['viewed', 'signed', 'declined', 'voided', 'expired', 'error'],
  viewed: ['signed', 'declined', 'voided', 'expired', 'error'],
  signed: ['completed', 'voided', 'expired', 'error'],
  completed: [],
  declined: [],
  voided: [],
  expired: [],
  error: [],
}

export function isSignatureRequestStatus(
  value: string,
): value is SignatureRequestStatus {
  return (SIGNATURE_REQUEST_STATUSES as readonly string[]).includes(value)
}

export function isActiveSignatureRequestStatus(
  status: SignatureRequestStatus,
): boolean {
  return (SIGNATURE_REQUEST_ACTIVE_STATUSES as readonly string[]).includes(status)
}

/** The canonical `signature_request` record (migration 036). Provider-free. */
export type SignatureRequest = {
  id: string
  transactionDocumentId: string
  status: SignatureRequestStatus
  message: string | null
  executionRole: string | null
  /** CRM-27 — the issued participant/signature slot this request satisfies. */
  executionSlotId: string | null
  createdByUserId: string | null
  createdAt: string
  updatedAt: string
}

// ---------------------------------------------------------------------------
// Neutral recipients (provider-neutral roles + signer ordering)
// ---------------------------------------------------------------------------

export const SIGNATURE_RECIPIENT_ROLES = ['signer', 'approver'] as const
export type SignatureRecipientRole = (typeof SIGNATURE_RECIPIENT_ROLES)[number]

export type SignatureRecipient = {
  /** Neutral role — never a provider-specific recipient concept. */
  role: SignatureRecipientRole
  name: string
  email: string
  /** Signer ordering (1-based); provider adapters honor it. */
  order: number
}

/** Transport-shape validation for the neutral send input (application owns authority). */
export function validateSignatureRecipients(
  recipients: readonly SignatureRecipient[],
): string[] {
  const errors: string[] = []
  if (recipients.length === 0) {
    errors.push('At least one recipient is required.')
    return errors
  }
  const seenEmails = new Set<string>()
  for (const r of recipients) {
    if (!(SIGNATURE_RECIPIENT_ROLES as readonly string[]).includes(r.role)) {
      errors.push(`Invalid recipient role: ${String(r.role)}.`)
    }
    if (!r.name.trim()) errors.push('Recipient name is required.')
    if (!r.email.trim()) errors.push('Recipient email is required.')
    if (!Number.isInteger(r.order) || r.order < 1) {
      errors.push('Recipient order must be a positive integer.')
    }
    const key = r.email.trim().toLowerCase()
    if (seenEmails.has(key)) {
      errors.push(`Duplicate recipient email: ${r.email}.`)
    }
    seenEmails.add(key)
  }
  return errors
}

// ---------------------------------------------------------------------------
// Neutral command inputs (the envelope `input` payloads)
// ---------------------------------------------------------------------------

export type SendSignatureRequestCommandInput = {
  transactionDocumentId: string
  /** Neutral recipients — flow through the seam to the provider; never stored
   *  on the canonical record (the canonical record is lifecycle-only). */
  recipients: SignatureRecipient[]
  message?: string | null
  createdByUserId?: string | null
  /** CRM-27 — the agreement execution role this request fulfills (provider-
   *  neutral; optional for requests not tied to an agreement role). */
  executionRole?: string | null
  /** CRM-27 — the issued participant/signature slot this request satisfies
   *  (e.g. "BUYER:1"); keyed to the immutable issued participant snapshot. */
  executionSlotId?: string | null
}

export type StatusSignatureRequestCommandInput = {
  signatureRequestId: string
  /**
   * Neutral target status observed at the seam (the provider status has
   * already been mapped). Absent = pure status read.
   */
  targetStatus?: SignatureRequestStatus | null
}

export type CancelSignatureRequestCommandInput = {
  signatureRequestId: string
}

export type DeclineSignatureRequestCommandInput = {
  signatureRequestId: string
}

// ---------------------------------------------------------------------------
// Neutral events (reuse the existing DomainEvent shape)
// ---------------------------------------------------------------------------

/**
 * Neutral DomainEvent types emitted at the seam. Downstream consumers
 * (DOC-05 reconciliation) subscribe to these neutral events, never to provider
 * webhooks. The status -> event mapping is the single source of truth.
 */
export const SIGNATURE_EVENT_TYPE_BY_STATUS: Partial<
  Record<SignatureRequestStatus, DomainEventType>
> = {
  sent: 'SIGNATURE_REQUEST_SENT',
  completed: 'SIGNATURE_REQUEST_COMPLETED',
  declined: 'SIGNATURE_REQUEST_DECLINED',
  voided: 'SIGNATURE_REQUEST_VOIDED',
}

// ---------------------------------------------------------------------------
// Provider interface contract (neutral surface; adapters implement it)
// ---------------------------------------------------------------------------

/**
 * Neutral webhook event vocabulary (the normalized form of a provider
 * webhook). The seam maps provider payloads onto these; the application never
 * sees provider envelope state.
 */
export const SIGNATURE_PROVIDER_EVENTS = [
  'sent',
  'viewed',
  'signed',
  'completed',
  'declined',
  'voided',
  'expired',
  'error',
] as const

export type SignatureProviderEvent = (typeof SIGNATURE_PROVIDER_EVENTS)[number]

/** The neutral send payload handed to the provider adapter. */
export type ProviderSendRequest = {
  /** NEUTRAL application signature request id — the provider adapter maps it
   *  to its own provider id in its provider table (DOC-04). */
  signatureRequestId: string
  transactionDocumentId: string
  recipients: SignatureRecipient[]
  message: string | null
}

/**
 * Outcome of a provider send. `providerStatus` is the RAW provider status
 * observed after send; the seam maps it to neutral at the boundary
 * (lib/signature/status-mapping.ts). Provider ids never cross the seam.
 */
export type ProviderSendResult =
  | { ok: true; providerStatus: string }
  | { ok: false; providerStatus: string; error: string }

/** Provider status observation, ALREADY mapped to neutral at the seam. */
export type ProviderStatusResult = {
  status: SignatureRequestStatus
}

export type ProviderActionResult = {
  ok: boolean
  error?: string
}

/**
 * Normalized webhook verification result: a neutral event plus the NEUTRAL
 * signature request id (the adapter resolves its provider id through its
 * provider table). Webhook handlers write through this, never straight to
 * transaction_document.
 */
export type WebhookVerificationResult = {
  event: SignatureProviderEvent
  signatureRequestId: string
}

// ---------------------------------------------------------------------------
// DOC-05 — Signed artifact download (the neutral surface of the one-time
// signed-bytes retrieval the reconciliation handler performs via DOC-04).
// ---------------------------------------------------------------------------

/**
 * The final signed artifact bytes, downloaded once from the provider (DOC-04
 * adapter) so DOC-05 reconciliation can append them as a NEW media row.
 * Provider-specific wire shapes never cross the seam — only these neutral
 * bytes and storage metadata do.
 */
export type SignedArtifactDownload = {
  /** The signed artifact bytes (PDF/document), appended as a NEW media row. */
  bytes: Uint8Array
  /** Storage filename for the signed artifact media row. */
  filename: string
  /** Storage mime type for the signed artifact media row. */
  mimeType: string
}
