// ---------------------------------------------------------------------------
// DOC-08 — TemplateDefinition registry (XML-backed, version-aware).
//
// XML is the CANONICAL template authoring format. Templates are parsed +
// validated into TemplateDefinition at load; the runtime consumes the seam —
// no layer ever reads raw XML. No database migration is involved: templates
// are versioned FILES, and template versioning is the <form version> attribute.
//
// VERSIONING (forms template versioning foundation):
//   - A form type may have MULTIPLE registered versions coexisting (e.g.
//     PR-PNS v1 and PR-PNS v2) so historical saved form instances are never
//     silently re-rendered against newer legal language.
//   - EXACT resolution is first-class: getTemplate(id, version) returns the
//     exact version or null — it NEVER falls forward to another version.
//   - getLatestTemplate(id) returns the highest registered version (used when
//     creating a NEW form).
//   - The pure helpers resolveTemplateVersion / resolveLatestTemplateVersion
//     operate on a defs array so tests can prove behavior under any population.
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

/**
 * The canonical authoring files (relative to the project templates dir).
 * A form type may span multiple files/versions; each file's <form version>
 * attribute declares its version. PR-PNS.xml is v1; PR-PNS.v2.xml is the
 * durable v2 placeholder (real legal content supplied separately by the human).
 */
const TEMPLATE_FILES = [
  'OFFER-01.xml',
  'PR-PNS.xml',
  'PR-PNS.v2.xml',
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

/** Pure: exact (id, version) resolution over a definition list. Never falls forward. */
export function resolveTemplateVersion(
  defs: readonly TemplateDefinition[],
  id: string,
  version: number,
): TemplateDefinition | null {
  return defs.find((t) => t.id === id && t.version === version) ?? null
}

/** Pure: highest registered version for an id over a definition list. */
export function resolveLatestTemplateVersion(
  defs: readonly TemplateDefinition[],
  id: string,
): TemplateDefinition | null {
  let best: TemplateDefinition | null = null
  for (const t of defs) {
    if (t.id === id && (best === null || t.version > best.version)) best = t
  }
  return best
}

/** EXACT resolution — returns the precise version or null (never another version). */
export function getTemplate(id: string, version: number): TemplateDefinition | null {
  return resolveTemplateVersion(TEMPLATES, id, version)
}

/** Highest registered version for a form type (used to create a NEW form). */
export function getLatestTemplate(id: string): TemplateDefinition | null {
  return resolveLatestTemplateVersion(TEMPLATES, id)
}

export function listTemplates(): readonly TemplateDefinition[] {
  return TEMPLATES
}

/** Form-type dropdown on the editor. All have XML in lib/forms/templates. */
const PORTAL_FORM_TYPES: readonly { id: string; displayName: string }[] = [
  { id: SHOWING_REPORT_TEMPLATE_ID, displayName: 'Showing Report' },
  { id: OFFER_LETTER_TEMPLATE_ID, displayName: 'Offer Letter' },
  { id: PURCHASE_SALE_TEMPLATE_ID, displayName: 'Purchase & Sale' },
  { id: LISTING_AGREEMENT_TEMPLATE_ID, displayName: 'Listing Contract' },
]

export function listPortalFormTypes(): readonly { id: string; displayName: string }[] {
  return PORTAL_FORM_TYPES.filter((item) => getLatestTemplate(item.id))
}
