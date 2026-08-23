// ---------------------------------------------------------------------------
// DOC-08 — TemplateDefinition registry (XML-backed).
//
// XML is the CANONICAL template authoring format. Templates are parsed +
// validated into TemplateDefinition at load; the runtime consumes the seam —
// no layer ever reads raw XML. No database migration is involved: templates
// are versioned FILES, and template versioning is the <form version> attribute.
// ---------------------------------------------------------------------------

import { readFileSync } from 'node:fs'
import { join } from 'node:path'

import { parseTemplateXml } from './xml-template'
import type { TemplateDefinition } from './template-types'

export const OFFER_LETTER_TEMPLATE_ID = 'OFFER-01'
export const PURCHASE_SALE_TEMPLATE_ID = 'PR-PNS'
export const PURCHASE_SALE_AMENDMENT_TEMPLATE_ID = 'PR-PNS-AMD'
export const LISTING_AGREEMENT_TEMPLATE_ID = 'LISTING-01'
export const SHOWING_INFO_TEMPLATE_ID = 'SHOW-INFO'
export const SHOWING_REPORT_TEMPLATE_ID = 'SHOW-RPT'

/** The canonical authoring files (relative to the project templates dir). */
const TEMPLATE_FILES = [
  'OFFER-01.xml',
  'PR-PNS.xml',
  'PR-PNS-AMD.xml',
  'LISTING-01.xml',
  'SHOW-INFO.xml',
  'SHOW-RPT.xml',
]

function loadTemplates(): TemplateDefinition[] {
  const dir = join(process.cwd(), 'lib', 'forms', 'templates')
  return TEMPLATE_FILES.map((file) => {
    const source = readFileSync(join(dir, file), 'utf8')
    try {
      return parseTemplateXml(source)
    } catch (error) {
      const detail = error instanceof Error ? error.message : String(error)
      throw new Error(`Template ${file} failed to load: ${detail}`)
    }
  })
}

const TEMPLATES: readonly TemplateDefinition[] = loadTemplates()

export function getTemplate(id: string): TemplateDefinition | null {
  return TEMPLATES.find((t) => t.id === id) ?? null
}

export function listTemplates(): readonly TemplateDefinition[] {
  return TEMPLATES
}

/** Form-type dropdown on the editor. All four have XML in lib/forms/templates. */
const PORTAL_FORM_TYPES: readonly { id: string; displayName: string }[] = [
  { id: SHOWING_REPORT_TEMPLATE_ID, displayName: 'Showing Report' },
  { id: OFFER_LETTER_TEMPLATE_ID, displayName: 'Offer Letter' },
  { id: PURCHASE_SALE_TEMPLATE_ID, displayName: 'Purchase & Sale' },
  { id: LISTING_AGREEMENT_TEMPLATE_ID, displayName: 'Listing Contract' },
]

export function listPortalFormTypes(): readonly { id: string; displayName: string }[] {
  return PORTAL_FORM_TYPES.filter((item) => getTemplate(item.id))
}
