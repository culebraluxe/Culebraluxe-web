// ---------------------------------------------------------------------------
// REL-INTEL — Apple Messages event → canonical interaction mapping (pure).
//
// The missing final leg: individual Apple message events are materialized into
// canonical `interaction` rows so Client Contact History reflects real
// chronology, NOT just aggregate evidence.
//
// PRIVACY BOUNDARY:
//   Persist ONLY a small bounded one-line memory cue (≤160 chars) as the
//   interaction summary. Never archive full transcripts, attributed bodies,
//   attachment contents, or attachment-filename dumps. Apple/Gmail remain the
//   source systems for full content; the CRM remembers the relationship moment.
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

/** Maximum length of the bounded one-line memory cue. */
export const APPLE_PREVIEW_MAX_LENGTH = 160

/**
 * Derive a bounded one-line memory cue from Apple message text.
 * - collapses whitespace / newlines to single spaces
 * - trims
 * - caps at ~160 characters
 * - returns null when there is no usable text (never fabricates prose)
 */
export function boundedPreview(text: string | null | undefined): string | null {
  if (!text) return null
  const oneLine = text.replace(/\s+/g, ' ').trim()
  if (!oneLine) return null
  if (oneLine.length <= APPLE_PREVIEW_MAX_LENGTH) return oneLine
  return `${oneLine.slice(0, APPLE_PREVIEW_MAX_LENGTH - 1).trimEnd()}…`
}

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
 * - event_type: 'message' (the canonical message event type).
 * - direction: inbound/outbound from Apple isFromMe.
 * - occurred_at: the real Apple event timestamp.
 * - source_system / source_external_id: stable Apple source + message GUID
 *   (replay key).
 * - summary: bounded one-line preview (≤160 chars), or a neutral
 *   "Message"/"Attachment" label when there is no usable text.
 * - source_metadata: minimal audit/provenance ONLY — never message content.
 */
export function mapAppleMessageToInteraction(
  message: AppleMessagesMessage,
  canonicalPersonId: string,
  sourceAccount: string,
): CreateInteractionInput {
  const direction = message.isFromMe === 1 ? 'outbound' : 'inbound'
  const hasText = Boolean(message.text?.trim())
  const summary = hasText
    ? boundedPreview(message.text)
    : message.hasAttachments === 1
      ? 'Attachment'
      : 'Message'
  return {
    personId: canonicalPersonId,
    channel: appleServiceToChannel(message.service),
    eventType: 'message',
    direction,
    occurredAt: message.dateISO ?? '',
    title: undefined,
    summary: summary ?? undefined,
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
