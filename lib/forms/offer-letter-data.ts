// ---------------------------------------------------------------------------
// DOC-07 — canonical-data prefill + form validation.
//
// PURE logic (no DB). The form page resolves a bounded deal-facts snapshot
// from the canonical workspace and this module maps it onto TemplateDefinition
// bindings. Missing canonical facts stay blank — the POC does NOT broaden
// domain models merely to fill a form.
// ---------------------------------------------------------------------------

import type {
  TemplateDefinition,
  TemplateFieldValues,
  TemplateSectionValues,
} from './template-types'

/** Bounded canonical facts available for prefill (resolved by the caller). */
export type DealFactsForForm = {
  clientName: string | null
  propertyLabel: string | null
  offerAmount: string | null
  financingType: string | null
  closingDate: string | null
  personDisplayName?: string | null
  propertyName?: string | null
  propertyLocation?: string | null
}

export function emptyDealFacts(): DealFactsForForm {
  return {
    clientName: null,
    propertyLabel: null,
    offerAmount: null,
    financingType: null,
    closingDate: null,
    personDisplayName: null,
    propertyName: null,
    propertyLocation: null,
  }
}

/** Resolve one binding path against the canonical facts. */
export function resolveBinding(
  binding: string,
  facts: DealFactsForForm,
): string | null {
  switch (binding) {
    case 'deal.client.name':
      return facts.clientName
    case 'deal.property.label':
      return facts.propertyLabel
    case 'deal.offer.amount':
      return facts.offerAmount
    case 'deal.financing.type':
      return facts.financingType
    case 'deal.closing.date':
      return facts.closingDate
    case 'person.displayName':
      return facts.personDisplayName ?? facts.clientName
    case 'property.name':
      return facts.propertyName ?? facts.propertyLabel
    case 'property.location':
      return facts.propertyLocation ?? null
    default:
      return null
  }
}

function isoDate(daysFromToday = 0): string {
  const date = new Date()
  date.setDate(date.getDate() + daysFromToday)
  return date.toISOString().slice(0, 10)
}

function defaultDateFor(fieldName: string): string {
  if (/expir/i.test(fieldName)) return isoDate(14)
  if (/end/i.test(fieldName)) return isoDate(90)
  return isoDate(0)
}

/** Human-approved template-owned defaults that are not domain facts. */
const TEMPLATE_FIELD_DEFAULTS: Readonly<Record<string, Readonly<Record<string, string>>>> = {
  'LISTING-01': {
    // CulebraLuxe listing agreements are brokered by Lisa. This identity must
    // be present before the protected local signature resolver is allowed to
    // compose Lisa's transparent signature image, initials and issuance date.
    brokerName: 'Lisa Penfield',
    // Current CulebraLuxe operating defaults. These remain editable on the form
    // but should not burden the operator on every new Listing Agreement.
    sellerCivilStatus: 'Single',
    commission: '4%',
    listingType: 'Exclusive Right to Sell',
  },
  'PR-PNS': {
    // Match the proven Listing Agreement broker mapping. CulebraLuxe is the
    // Seller's Broker on the standard P&S and Lisa occupies that execution
    // role locally before the external BoldSign envelope is constructed.
    sellerBrokerName: 'Lisa Penfield',
  },
}

/** Fill empty date fields so the native date input is a real value, not a grey placeholder. */
export function applyDateDefaults(
  template: TemplateDefinition,
  values: TemplateFieldValues,
): TemplateFieldValues {
  const next = { ...values }
  for (const field of template.fields) {
    if (field.type !== 'date') continue
    if ((next[field.name] ?? '').trim()) continue
    next[field.name] = defaultDateFor(field.name)
  }
  return next
}

/**
 * Prefill a field-values map for a template from canonical facts. Bound fields
 * adopt the canonical value; unbound fields start blank unless the template has
 * a human-approved business default. Dates receive their existing date default.
 */
export function prefillFieldValues(
  template: TemplateDefinition,
  facts: DealFactsForForm,
): TemplateFieldValues {
  const values: TemplateFieldValues = {}
  const defaults = TEMPLATE_FIELD_DEFAULTS[template.id] ?? {}
  for (const field of template.fields) {
    if (!field.binding) {
      values[field.name] = defaults[field.name] ?? ''
      continue
    }
    values[field.name] = resolveBinding(field.binding, facts) ?? defaults[field.name] ?? ''
  }
  return applyDateDefaults(template, values)
}

export function emptySectionValues(
  template: TemplateDefinition,
): TemplateSectionValues {
  const sections: TemplateSectionValues = {}
  for (const section of template.sections) {
    sections[section.name] = ''
  }
  return sections
}

export type FormValidationIssue = {
  field: string
  label: string
  message: string
}

/**
 * Required markers are operator guidance, not a write gate. CulebraLuxe forms
 * must always remain savable/issuable while facts are being assembled. The UI
 * continues to show required-field markers, but missing values never block the
 * operator's Save, Share, or signature workflow.
 */
export function validateFormValues(
  _template: TemplateDefinition,
  _values: TemplateFieldValues,
): FormValidationIssue[] {
  return []
}
