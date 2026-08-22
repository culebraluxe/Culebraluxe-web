// ---------------------------------------------------------------------------
// DOC-07 — TemplateDefinition registry.
//
// The runtime consumes TemplateDefinition through this seam — never by
// hardcoding semantics inside a page component. The POC registers the single
// Offer Letter fixture; the future human-editable format plugs in here.
// ---------------------------------------------------------------------------

import { OFFER_LETTER_TEMPLATE, OFFER_LETTER_TEMPLATE_ID } from './offer-letter'
import type { TemplateDefinition } from './template-types'

const TEMPLATES: readonly TemplateDefinition[] = [OFFER_LETTER_TEMPLATE]

export function getTemplate(id: string): TemplateDefinition | null {
  return TEMPLATES.find((t) => t.id === id) ?? null
}

export function listTemplates(): readonly TemplateDefinition[] {
  return TEMPLATES
}

export { OFFER_LETTER_TEMPLATE_ID }
