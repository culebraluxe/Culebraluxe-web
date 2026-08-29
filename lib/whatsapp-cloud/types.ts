import type { JsonObject } from '../crm-types'

export type MetaWhatsAppContact = {
  wa_id?: string
  profile?: { name?: string }
}

export type MetaWhatsAppMessage = {
  from?: string
  to?: string
  id?: string
  timestamp?: string
  type?: string
  context?: { id?: string }
  image?: { id?: string; mime_type?: string; caption?: string }
  video?: { id?: string; mime_type?: string; caption?: string }
  audio?: { id?: string; mime_type?: string }
  document?: {
    id?: string
    mime_type?: string
    filename?: string
    caption?: string
  }
  sticker?: { id?: string; mime_type?: string }
  text?: { body?: string }
  button?: JsonObject
  interactive?: JsonObject
  location?: JsonObject
  contacts?: JsonObject[]
}

export type MetaWhatsAppChangeValue = {
  messaging_product?: string
  metadata?: {
    display_phone_number?: string
    phone_number_id?: string
  }
  contacts?: MetaWhatsAppContact[]
  messages?: MetaWhatsAppMessage[]
  message_echoes?: MetaWhatsAppMessage[]
  statuses?: unknown[]
}

export type MetaWhatsAppWebhookPayload = {
  object?: string
  entry?: Array<{
    id?: string
    changes?: Array<{
      field?: string
      value?: MetaWhatsAppChangeValue
    }>
  }>
}

export type MetaWhatsAppConfiguration = {
  appSecret: string
  phoneNumberId: string
  ownedPhoneE164: string
}
