// ---------------------------------------------------------------------------
// DOC-04 — BoldSign Integration: webhook event -> neutral event normalization.
//
// Canonical signature_request status is ENVELOPE-level. BoldSign's Viewed and
// Signed callbacks are recipient-level activity in a multi-party envelope;
// neither proves that the whole agreement has been executed. Both therefore
// remain on the active `viewed` plateau until BoldSign emits Completed.
//
// Events that indicate an operator-visible delivery/authentication/identity
// problem fail closed rather than being silently normalized to an ordinary
// active state. The webhook endpoint then returns non-2xx, preserving provider
// retry/operational visibility while the canonical request remains active and
// cannot be replaced by a second legal envelope.
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
  // `Signed` means ONE recipient signed; it is not whole-envelope execution.
  Signed: 'viewed',
  Completed: 'completed',
  Declined: 'declined',
  Revoked: 'voided',
  Expired: 'expired',
  SendFailed: 'error',
}

const BOLD_SIGN_ATTENTION_EVENTS = new Set([
  'DeliveryFailed',
  'AuthenticationFailed',
  'IdentityVerificationFailed',
  'KBAFailed',
  // Reassignment is explicitly disabled on send. Observing it anyway is an
  // immutable-recipient invariant breach and must never be silently accepted.
  'Reassigned',
])

/**
 * Normalize a BoldSign webhook event onto the neutral provider event
 * vocabulary. Direct lifecycle mappings are exact. Recipient-level activity
 * stays active until Completed. Attention events throw intentionally so a
 * delivery/authentication/identity problem is visible and the current legal
 * envelope remains the sole active request.
 *
 * Benign events with no lifecycle meaning (Reminder, SignerSaved, Edited, ...)
 * may fall back to the document's ACTIVE status only. Unknown or terminal
 * fallbacks fail closed rather than fabricating a terminal outcome.
 */
export function mapBoldSignWebhookEvent(
  eventType: string,
  documentStatus: string | null,
): SignatureProviderEvent {
  if (BOLD_SIGN_ATTENTION_EVENTS.has(eventType)) {
    throw new Error(
      `BoldSign webhook event ${JSON.stringify(eventType)} requires operator attention; ` +
        'the signature envelope remains active and must not be replaced automatically.',
    )
  }

  const direct = BOLD_SIGN_WEBHOOK_EVENT_TO_NEUTRAL[eventType]
  if (direct) return direct

  if (documentStatus !== null) {
    const fromStatus = mapProviderStatus(BOLD_SIGN_PROVIDER, documentStatus)
    if (isActiveSignatureRequestStatus(fromStatus) && fromStatus !== 'requested') {
      // Normalize all nonterminal provider activity to the same active plateau;
      // this prevents late/out-of-order recipient callbacks from moving the
      // envelope backwards (for example Signed signer #1 then Viewed signer #2).
      return 'viewed'
    }
  }

  throw new Error(
    `BoldSign webhook event ${JSON.stringify(eventType)} has no neutral lifecycle mapping.`,
  )
}
