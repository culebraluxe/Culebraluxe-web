import { normalizePhone } from '../crm-intake-normalization'
import type {
  ExternalActivityEvent,
  ExternalAttachment,
} from '../mac-observer/contracts'

import type {
  MetaWhatsAppChangeValue,
  MetaWhatsAppMessage,
  MetaWhatsAppWebhookPayload,
} from './types'

const MESSAGE_TYPE = /^[a-z0-9_]{1,64}$/

function metaPhone(value: string | undefined, field: string): string {
  const digits = value?.replace(/[^\d]/g, '')
  if (!digits) throw new Error(`${field} is missing.`)
  return normalizePhone(`+${digits}`)
}

function occurredAt(value: string | undefined): string {
  if (!value || !/^\d+$/.test(value)) {
    throw new Error('WhatsApp message timestamp is invalid.')
  }
  const date = new Date(Number(value) * 1000)
  if (Number.isNaN(date.getTime())) {
    throw new Error('WhatsApp message timestamp is invalid.')
  }
  return date.toISOString()
}

function messageType(value: string | undefined): string {
  const normalized = value?.trim().toLowerCase() || 'unknown'
  return MESSAGE_TYPE.test(normalized) ? normalized : 'unknown'
}

function attachments(message: MetaWhatsAppMessage): ExternalAttachment[] {
  const media = [
    message.image,
    message.video,
    message.audio,
    message.document,
    message.sticker,
  ].find((candidate) => candidate?.id)
  if (!media?.id) return []
  return [
    {
      referenceId: media.id,
      mimeType: media.mime_type,
      filename:
        message.document?.id === media.id
          ? message.document.filename
          : undefined,
    },
  ]
}

function displayName(
  value: MetaWhatsAppChangeValue,
  externalPhone: string,
): string | undefined {
  const digits = externalPhone.replace(/^\+/, '')
  return value.contacts?.find((contact) => contact.wa_id === digits)?.profile
    ?.name
}

function normalizedEvent(input: {
  message: MetaWhatsAppMessage
  value: MetaWhatsAppChangeValue
  phoneNumberId: string
  ownedPhoneE164: string
  direction: 'inbound' | 'outbound'
  observedAt: string
}): ExternalActivityEvent {
  const { message, value, direction, observedAt } = input
  if (!message.id?.trim()) throw new Error('WhatsApp message id is missing.')

  const externalPhone =
    direction === 'inbound'
      ? metaPhone(message.from, 'WhatsApp sender')
      : metaPhone(message.to, 'WhatsApp recipient')
  const name = displayName(value, externalPhone)
  const type = messageType(message.type)
  const external = {
    kind: 'phone' as const,
    value: externalPhone,
    ...(name ? { displayName: name } : {}),
  }
  const owned = { kind: 'phone' as const, value: input.ownedPhoneE164 }
  const sender = direction === 'inbound' ? external : owned
  const recipient = direction === 'inbound' ? owned : external
  const media = attachments(message)

  return {
    schemaVersion: 1,
    source: 'whatsapp',
    sourceAccount: `meta-${input.phoneNumberId}`,
    externalEventId: message.id,
    eventType: `whatsapp.message_${direction === 'inbound' ? 'received' : 'sent'}.${type}`,
    occurredAt: occurredAt(message.timestamp),
    observedAt,
    direction,
    participants: [
      { ...sender, role: 'sender' },
      { ...recipient, role: 'recipient' },
    ],
    contactCandidates: [external],
    thread: message.context?.id
      ? { id: message.context.id, inReplyTo: message.context.id }
      : undefined,
    // Metadata-only retention: never copy text/captions or the raw webhook.
    content: undefined,
    attachments: media.length ? media : undefined,
    correlationId: undefined,
    context: undefined,
    provenance: {
      adapter: 'meta-whatsapp-cloud',
      adapterVersion: 'meta-whatsapp-cloud.v1',
    },
  }
}

export function parseMetaWhatsAppWebhook(input: {
  payload: MetaWhatsAppWebhookPayload
  phoneNumberId: string
  ownedPhoneE164: string
  observedAt?: string
}): ExternalActivityEvent[] {
  if (input.payload.object !== 'whatsapp_business_account') return []
  const observedAt = input.observedAt ?? new Date().toISOString()
  const events: ExternalActivityEvent[] = []

  for (const entry of input.payload.entry ?? []) {
    for (const change of entry.changes ?? []) {
      const value = change.value
      if (!value || value.messaging_product !== 'whatsapp') continue
      if (value.metadata?.phone_number_id !== input.phoneNumberId) continue

      for (const message of value.messages ?? []) {
        events.push(normalizedEvent({
          message,
          value,
          phoneNumberId: input.phoneNumberId,
          ownedPhoneE164: input.ownedPhoneE164,
          direction: 'inbound',
          observedAt,
        }))
      }
      for (const message of value.message_echoes ?? []) {
        events.push(normalizedEvent({
          message,
          value,
          phoneNumberId: input.phoneNumberId,
          ownedPhoneE164: input.ownedPhoneE164,
          direction: 'outbound',
          observedAt,
        }))
      }
    }
  }
  return events
}
