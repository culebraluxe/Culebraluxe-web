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
  | 'person.displayName'
  | 'property.name'
  | 'property.location'
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
  /**
   * DOC-08 — ordered default-text segments: literal text plus declared
   * `<value field="X"/>` substitutions. Boilerplate legal text lives here in
   * the XML template; the runtime DRAFT (JSONB) is plain text and — when
   * non-empty — takes precedence at render time (editable sections).
   */
  segments: TemplateSectionSegment[]
  /** Field ids referenced by `<value>` segments (validated to exist). */
  values: string[]
}

/** One ordered default-text segment inside a section. */
export type TemplateSectionSegment =
  | { kind: 'text'; text: string }
  | { kind: 'value'; field: string }

/** Rendering metadata sufficient for the POC PDF renderer + preview. */
export type TemplatePresentation =
  | 'agreement'
  | 'letter'
  | 'information'
  | 'report'

export type TemplateRendering = {
  /** PDF document title (e.g. 'OFFER LETTER'). */
  title: string
  /** Party heading / issuer line. */
  issuer: string
  /**
   * Bounded document-composition profile. This is a presentation hint, not a
   * form-specific renderer or executable template logic.
   */
  presentation: TemplatePresentation
}

/**
 * DOC-08/DOC-09 — a role-driven participant collection (BUYER, SELLER,
 * BUYER_BROKER, SELLER_BROKER, ...). Never buyer1/buyer2 hardcoding; multiple
 * participants per role are allowed. DOC-09/10 consume this structure.
 */
export type TemplateParticipantRole = {
  /** Stable role code (e.g. 'BUYER'). */
  role: string
  label: string
  /** Whether more than one participant may hold this role. */
  multiple: boolean
}

/**
 * DOC-08/DOC-09 — a signature block for a participant role. `field` names the
 * form field carrying the signer's displayed name (validated to exist).
 */
export type TemplateSignatureGroup = {
  role: string
  label: string
  /** Form field id whose value names the signer (null = no name line). */
  field: string | null
  /** Whether initials are required on the block. */
  initials: boolean
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
  /** DOC-08/DOC-09 — role-driven participant collections (may be empty). */
  participants: TemplateParticipantRole[]
  /** DOC-08/DOC-09 — signature blocks (may be empty). */
  signatureGroups: TemplateSignatureGroup[]
}

/** A field value map keyed by TemplateFieldDefinition.name. */
export type TemplateFieldValues = Record<string, string>

/** A prose section map keyed by TemplateSectionDefinition.name. */
export type TemplateSectionValues = Record<string, string>
