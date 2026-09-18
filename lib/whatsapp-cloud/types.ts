// Application types for the WhatsApp Cloud API webhook are INFERRED from the
// runtime schema in ./schema.ts. There is no hand-written payload shape to keep
// in step with the validator: change the schema and these types follow.
export type {
  MetaWhatsAppChangeValue,
  MetaWhatsAppContact,
  MetaWhatsAppMessage,
  MetaWhatsAppWebhookPayload,
} from './schema'

export type MetaWhatsAppConfiguration = {
  appSecret: string
  phoneNumberId: string
  ownedPhoneE164: string
}
