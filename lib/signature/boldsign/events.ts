// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration: webhook event -> neutral event normalization.
//
// BoldSign webhook DOCUMENT events that map DIRECTLY onto a neutral lifecycle
// event are mapped exactly. Events with no lifecycle meaning (Reminder,
// Reassigned, SignerSaved, Edited, EditFailed, DeliveryFailed, ...) fall back
// to the document's own status (data.status), which the document-status map
// (lib/signature/status-mapping.ts) already normalizes — for a sent envelope
// that is 'InProgress' -> neutral 'sent', a safe no-op re-application. A
// payload whose event maps to neither fails closed (the seam cannot represent
// it as a status transition).
//
// ALL BoldSign vocabulary stays inside the adapter: only the NEUTRAL event
// crosses the seam.
// ---------------------------------------------------------------------------

import type { SignatureProviderEvent } from '../contracts'
import { isActiveSignatureRequestStatus } from '../contracts'
import {
  BOLD_SIGN_PROVIDER,
  mapProviderStatus,
} from '../status-mapping'

const BOLD_SIGN_WEBHOOK_EVENT_TO_NEUTRAL: Partial<Record<string, SignatureProviderEvent>> = {
  Sent: 'sent',
  Viewed: 'viewed',
  Signed: 'signed',
  Completed: 'completed',
  Declined: 'declined',
  Revoked: 'voided',
  Expired: 'expired',
  SendFailed: 'error',
}

/**
 * Normalize a BoldSign webhook event onto the neutral provider event
 * vocabulary. Direct eventType mappings are exact. Events with no lifecycle
 * meaning (Reminder, Reassigned, SignerSaved, ...) fall back to the
 * document's own status — but ONLY when that status maps to an ACTIVE neutral
 * state (sent/viewed/signed), i.e. a safe no-op/forward move for a request
 * that is in flight. A benign notification must never terminate a healthy
 * request: an unknown or terminal fallback fails closed (throw) rather than
 * fabricating a status the seam cannot safely apply.
 */
export function mapBoldSignWebhookEvent(
  eventType: string,
  documentStatus: string | null,
): SignatureProviderEvent {
  const direct = BOLD_SIGN_WEBHOOK_EVENT_TO_NEUTRAL[eventType]
  if (direct) return direct
  if (documentStatus !== null) {
    const fromStatus = mapProviderStatus(BOLD_SIGN_PROVIDER, documentStatus)
    // Only ACTIVE neutral states are safe fallbacks (a pre-send 'requested'
    // is never a webhook event; 'error'/'completed'/... would fabricate a
    // terminal outcome from a notification that carries no lifecycle meaning).
    if (isActiveSignatureRequestStatus(fromStatus) && fromStatus !== 'requested') {
      return fromStatus
    }
  }
  throw new Error(
    `BoldSign webhook event ${JSON.stringify(eventType)} has no neutral lifecycle mapping.`,
  )
}
