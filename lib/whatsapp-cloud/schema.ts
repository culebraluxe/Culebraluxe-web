import { z } from 'zod'

// -----------------------------------------------------------------------------
// The WhatsApp Cloud API webhook payload crosses a process boundary: it is
// untrusted input. This module owns the ONE runtime schema for it, and the
// application types are INFERRED from that schema (see ./types.ts) so the
// validator and the type cannot drift. Every object is loose (`.passthrough()`)
// because Meta adds fields without notice — an over-strict schema would reject
// real traffic. Only a wrong-typed field fails validation.
// -----------------------------------------------------------------------------

const jsonObjectSchema = z.record(z.unknown())

export const metaWhatsAppContactSchema = z
  .object({
    wa_id: z.string().optional(),
    profile: z.object({ name: z.string().optional() }).passthrough().optional(),
  })
  .passthrough()

export const metaWhatsAppMessageSchema = z
  .object({
    from: z.string().optional(),
    to: z.string().optional(),
    id: z.string().optional(),
    timestamp: z.string().optional(),
    type: z.string().optional(),
    context: z.object({ id: z.string().optional() }).passthrough().optional(),
    image: z
      .object({
        id: z.string().optional(),
        mime_type: z.string().optional(),
        caption: z.string().optional(),
      })
      .passthrough()
      .optional(),
    video: z
      .object({
        id: z.string().optional(),
        mime_type: z.string().optional(),
        caption: z.string().optional(),
      })
      .passthrough()
      .optional(),
    audio: z
      .object({
        id: z.string().optional(),
        mime_type: z.string().optional(),
      })
      .passthrough()
      .optional(),
    document: z
      .object({
        id: z.string().optional(),
        mime_type: z.string().optional(),
        filename: z.string().optional(),
        caption: z.string().optional(),
      })
      .passthrough()
      .optional(),
    sticker: z
      .object({
        id: z.string().optional(),
        mime_type: z.string().optional(),
      })
      .passthrough()
      .optional(),
    text: z.object({ body: z.string().optional() }).passthrough().optional(),
    button: jsonObjectSchema.optional(),
    interactive: jsonObjectSchema.optional(),
    location: jsonObjectSchema.optional(),
    contacts: z.array(jsonObjectSchema).optional(),
  })
  .passthrough()

export const metaWhatsAppChangeValueSchema = z
  .object({
    messaging_product: z.string().optional(),
    metadata: z
      .object({
        display_phone_number: z.string().optional(),
        phone_number_id: z.string().optional(),
      })
      .passthrough()
      .optional(),
    contacts: z.array(metaWhatsAppContactSchema).optional(),
    messages: z.array(metaWhatsAppMessageSchema).optional(),
    message_echoes: z.array(metaWhatsAppMessageSchema).optional(),
    statuses: z.array(z.unknown()).optional(),
  })
  .passthrough()

export const metaWhatsAppWebhookPayloadSchema = z
  .object({
    object: z.string().optional(),
    entry: z
      .array(
        z
          .object({
            id: z.string().optional(),
            changes: z
              .array(
                z
                  .object({
                    field: z.string().optional(),
                    value: metaWhatsAppChangeValueSchema.optional(),
                  })
                  .passthrough(),
              )
              .optional(),
          })
          .passthrough(),
      )
      .optional(),
  })
  .passthrough()

export type MetaWhatsAppContact = z.infer<typeof metaWhatsAppContactSchema>
export type MetaWhatsAppMessage = z.infer<typeof metaWhatsAppMessageSchema>
export type MetaWhatsAppChangeValue = z.infer<typeof metaWhatsAppChangeValueSchema>
export type MetaWhatsAppWebhookPayload = z.infer<
  typeof metaWhatsAppWebhookPayloadSchema
>

export type MetaWhatsAppPayloadValidation =
  | { ok: true; payload: MetaWhatsAppWebhookPayload }
  | { ok: false; issues: string[] }

/**
 * Validate untrusted input at the process boundary. Callers must treat a
 * failure as a rejected request, never as a recoverable payload.
 */
export function validateMetaWhatsAppWebhookPayload(
  value: unknown,
): MetaWhatsAppPayloadValidation {
  const result = metaWhatsAppWebhookPayloadSchema.safeParse(value)
  if (result.success) return { ok: true, payload: result.data }
  return {
    ok: false,
    issues: result.error.issues.map(
      (issue) => `${issue.path.join('.') || '(root)'}: ${issue.message}`,
    ),
  }
}
