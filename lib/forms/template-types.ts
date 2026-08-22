// ---------------------------------------------------------------------------
// DOC-07 — TemplateDefinition seam: types.
//
// A deliberately small abstraction. The runtime consumes TemplateDefinition
// (never hardcoded page semantics) so a future story can swap in a small
// human-editable declarative template format behind this seam. This is NOT a
// template-management system, NOT a form-builder framework, NOT XSLT, NOT a
// schema compiler.
//
// POC scope: a simple adapter/fixture (lib/forms/offer-letter.ts) implements
// the seam; the renderer and the form UI both read from it.
// ---------------------------------------------------------------------------

/** Basic field types the POC renderer + form UI understand. */
export type TemplateFieldType =
  | 'text'
  | 'money'
  | 'date'
  | 'textarea'
  | 'select'

/**
 * Optional canonical-data binding. A small, bounded vocabulary resolved by
 * lib/forms/offer-letter-data.ts against a deal facts snapshot; anything not
 * bound stays a plain user-entered field for the POC. Do not broaden the
 * domain model merely to fill a form.
 */
export type TemplateFieldBinding =
  | 'deal.client.name'
  | 'deal.property.label'
  | 'deal.offer.amount'
  | 'deal.financing.type'
  | 'deal.closing.date'
  | null

export type TemplateFieldDefinition = {
  /** Stable machine name (the form-instance JSON key). */
  name: string
  /** Human label (e.g. 'Offer amount'). */
  label: string
  type: TemplateFieldType
  required: boolean
  /** Canonical-data binding when available; null = user-entered. */
  binding?: TemplateFieldBinding
  /** Select options when type === 'select'. */
  options?: readonly string[]
}

/** Bounded editable prose section (e.g. Special Terms). */
export type TemplateSectionDefinition = {
  /** Stable machine name (the form-instance JSON key). */
  name: string
  label: string
  editable: boolean
}

/** Rendering metadata sufficient for the POC PDF renderer + preview. */
export type TemplateRendering = {
  /** PDF document title (e.g. 'OFFER LETTER'). */
  title: string
  /** Party heading / issuer line. */
  issuer: string
}

/**
 * One approved transaction form. `id` is the stable machine identity used by
 * the form-instance table and the issued-document lineage.
 */
export type TemplateDefinition = {
  id: string
  version: number
  displayName: string
  /** Canonical document type label carried onto transaction_document. */
  documentTypeLabel: string
  fields: TemplateFieldDefinition[]
  sections: TemplateSectionDefinition[]
  rendering: TemplateRendering
}

/** A field value map keyed by TemplateFieldDefinition.name. */
export type TemplateFieldValues = Record<string, string>

/** A prose section map keyed by TemplateSectionDefinition.name. */
export type TemplateSectionValues = Record<string, string>
