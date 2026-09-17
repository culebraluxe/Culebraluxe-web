// ---------------------------------------------------------------------------
// Pure mapping from a Meta WhatsApp change envelope to the landing input.
//
// WHY THIS IS NOT IN THE ROUTE: a Next.js route module may export only HTTP
// methods and config — the generated guard is
// `checkFields<Diff<{ GET?: Function, ... }, TEntry>>` — so a pure helper
// exported from `route.ts` fails `next build` / `next typegen`. The route calls
// this; the test imports it directly.
//
// The mapping reads the ENVELOPE, not the phone: `to_address` is the receiving
// business line (`metadata.display_phone_number`), `source_account` stays the
// phone_number_id, and `conversation_id` is a stable thread key derived from
// the receiving line and the contact. The quoted reply id is kept separately in
// `context_id`, and `raw` carries the envelope facts actually read.
// ---------------------------------------------------------------------------

import type { LandedWhatsapp } from '../../db/landing'

import type {
  MetaWhatsAppChangeValue,
  MetaWhatsAppContact,
  MetaWhatsAppMessage,
} from './types'

export type WhatsAppMessageDirection = 'incoming' | 'outgoing'

export type WhatsAppLandingMappingInput = {
  /** The `change.value` envelope the message was read from. */
  value: MetaWhatsAppChangeValue | null | undefined
  message: MetaWhatsAppMessage
  direction: WhatsAppMessageDirection
  /** Fallback when the envelope carries no phone_number_id (fixtures). */
  sourceAccount: string | null
}

function address(value: string | null | undefined): string | null {
  const trimmed = (value ?? '').trim()
  return trimmed.length > 0 ? trimmed : null
}

function mediaId(message: MetaWhatsAppMessage): string | null {
  return (
    message.image?.id ??
    message.video?.id ??
    message.audio?.id ??
    message.document?.id ??
    message.sticker?.id ??
    null
  )
}

function sentAt(value: string | undefined): string | null {
  if (!value) return null
  const seconds = Number(value)
  if (!Number.isFinite(seconds)) return null
  const date = new Date(seconds * 1000)
  return Number.isNaN(date.getTime()) ? null : date.toISOString()
}

/**
 * The stable thread key: the receiving line plus the counterparty, so two
 * messages between the same pair share one thread regardless of direction.
 * Deliberately NOT the quoted reply id — that is a per-message fact.
 */
export function whatsappThreadKey(
  receivingLine: string | null,
  counterparty: string | null,
): string | null {
  if (!receivingLine || !counterparty) return null
  return `wa:${receivingLine}:${counterparty}`
}

function matchingContact(
  contacts: MetaWhatsAppContact[] | undefined,
  counterparty: string | null,
): MetaWhatsAppContact | null {
  if (!contacts || contacts.length === 0) return null
  if (counterparty) {
    const exact = contacts.find((contact) => contact.wa_id === counterparty)
    if (exact) return exact
  }
  return contacts[0] ?? null
}

export function mapWhatsAppMessageToLanding(
  input: WhatsAppLandingMappingInput,
): LandedWhatsapp {
  const { value, message, direction } = input
  const metadata = value?.metadata ?? null
  const receivingLine = address(metadata?.display_phone_number)
  const sourceAccount =
    address(metadata?.phone_number_id) ?? address(input.sourceAccount)
  const counterparty =
    direction === 'outgoing' ? address(message.to) : address(message.from)
  const contact = matchingContact(value?.contacts, counterparty)

  return {
    sourceAccount,
    sourceMessageId: message.id ?? '',
    conversationId: whatsappThreadKey(receivingLine, counterparty),
    contextId: message.context?.id ?? null,
    fromAddress: address(message.from),
    toAddress: receivingLine,
    direction,
    messageType: message.type ?? null,
    text: message.text?.body ?? null,
    mediaId: mediaId(message),
    sentAt: sentAt(message.timestamp),
    // The envelope facts actually read: the change metadata and the matching
    // contact. Never the whole Meta payload, never the bare message object.
    raw: { metadata, contact },
  }
}
