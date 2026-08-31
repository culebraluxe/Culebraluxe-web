// ---------------------------------------------------------------------------
// DOC-07 — TemplateDefinition seam: types.
// ---------------------------------------------------------------------------

export type TemplateFieldType =
  | 'text'
  | 'money'
  | 'date'
  | 'textarea'
  | 'select'

/** Visibility gate parsed from XML when="field:Value,Value". */
export type TemplateWhen = {
  field: string
  values: readonly string[]
}

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
  name: string
  label: string
  type: TemplateFieldType
  required: boolean
  binding?: TemplateFieldBinding
  options?: readonly string[]
  when?: TemplateWhen | null
}

export type TemplateSectionDefinition = {
  name: string
  label: string
  editable: boolean
  segments: TemplateSectionSegment[]
  values: string[]
  when?: TemplateWhen | null
}

export type TemplateSectionSegment =
  | { kind: 'text'; text: string }
  | { kind: 'value'; field: string }

export type TemplatePresentation =
  | 'agreement'
  | 'letter'
  | 'information'
  | 'report'

export type TemplateRendering = {
  title: string
  issuer: string
  presentation: TemplatePresentation
}

export type TemplateParticipantRole = {
  role: string
  label: string
  multiple: boolean
}

export type TemplateSignatureGroup = {
  role: string
  label: string
  field: string | null
  initials: boolean
}

export type TemplateDefinition = {
  id: string
  version: number
  displayName: string
  documentTypeLabel: string
  fields: TemplateFieldDefinition[]
  sections: TemplateSectionDefinition[]
  rendering: TemplateRendering
  participants: TemplateParticipantRole[]
  signatureGroups: TemplateSignatureGroup[]
}

export type TemplateFieldValues = Record<string, string>
export type TemplateSectionValues = Record<string, string>
