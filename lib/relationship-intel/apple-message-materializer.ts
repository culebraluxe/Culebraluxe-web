// ---------------------------------------------------------------------------
// REL-INTEL — Apple Messages event → canonical interaction mapping (pure).
//
// The missing final leg: individual Apple message events are materialized into
// canonical `interaction` rows so Client Contact History reflects real
// chronology, NOT just aggregate evidence.
//
// NON-NEGOTIABLE PRIVACY RULE:
//   NEVER persist message text / attributed text / snippets / attachment
//   contents / attachment filenames. The CRM remembers THAT a communication
//   happened, through which channel, when, and in which direction — never the
//   private prose. Apple/Gmail remain the source systems for content.
//
// The stable per-message identifier is the Apple `guid`. That becomes
// interaction.source_external_id, which together with the stable
// `source_system` ('apple_messages') is the replay key backed by the existing
// unique partial index interaction_source_identity_unique.
// ---------------------------------------------------------------------------
import type { CreateInteractionInput } from '../crm-types'
import {
  APPLE_MESSAGES_SOURCE,
  type AppleMessagesMessage,
} from './apple-messages'

/** Map an Apple service value to the canonical interaction channel. */
export function appleServiceToChannel(
  service: string | null,
): 'imessage' | 'sms' {
  const s = (service ?? '').toLowerCase()
  return s.includes('sms') ? 'sms' : 'imessage'
}

/**
 * Map one reconciled Apple message event to a canonical interaction input.
 *
 * - personId: the reconciled canonical Person (only ever supplied by the
 *   caller AFTER an authoritative exact_link reconcile decision).
 * - channel: iMessage / SMS per Apple service evidence.
 * - event_type: 'message' (the canonical message event type; no prose).
 * - direction: inbound/outbound from Apple isFromMe.
 * - occurred_at: the real Apple event timestamp.
 * - source_system / source_external_id: stable Apple source + message GUID
 *   (replay key).
 * - source_metadata: minimal audit/provenance ONLY — never message content.
 * - title/summary: intentionally null (no private prose).
 */
export function mapAppleMessageToInteraction(
  message: AppleMessagesMessage,
  canonicalPersonId: string,
  sourceAccount: string,
): CreateInteractionInput {
  const direction = message.isFromMe === 1 ? 'outbound' : 'inbound'
  return {
    personId: canonicalPersonId,
    channel: appleServiceToChannel(message.service),
    eventType: 'message',
    direction,
    occurredAt: message.dateISO ?? '',
    title: undefined,
    summary: undefined,
    sourceSystem: APPLE_MESSAGES_SOURCE,
    sourceExternalId: message.guid,
    sourceMetadata: {
      sourceAccount,
      handleId: message.handleId ?? null,
      service: message.service ?? null,
      hasAttachments: message.hasAttachments === 1,
    },
  }
}

/** Reconcile-state lookup keyed by Apple handle identity. */
export type HandleReconcileState = {
  reviewState: string
  canonicalPersonId: string | null
}

export type HandlePersonResolution =
  | { ok: true; canonicalPersonId: string }
  | { ok: false; reason: 'no_evidence' | 'not_exact_linked' | 'no_canonical_person' }

/**
 * Resolve whether an Apple handle may be materialized. Only an AUTHORITATIVE
 * canonical linkage (review_state = 'exact_linked' with a canonical_person_id)
 * qualifies. Ambiguous / unmatched / deferred / review_required handles are
 * never silently attached to a person.
 */
export function resolveHandlePerson(
  byIdentityKey: ReadonlyMap<string, HandleReconcileState>,
  handleId: string,
): HandlePersonResolution {
  const ev = byIdentityKey.get(handleId)
  if (!ev) return { ok: false, reason: 'no_evidence' }
  if (ev.reviewState !== 'exact_linked') return { ok: false, reason: 'not_exact_linked' }
  if (!ev.canonicalPersonId) return { ok: false, reason: 'no_canonical_person' }
  return { ok: true, canonicalPersonId: ev.canonicalPersonId }
}
