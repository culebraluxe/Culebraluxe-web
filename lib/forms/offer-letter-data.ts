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
 * adopt the canonical value; unbound fields start blank (user-entered) except
 * dates, which default to today so the input is actually set.
 */
export function prefillFieldValues(
  template: TemplateDefinition,
  facts: DealFactsForForm,
): TemplateFieldValues {
  const values: TemplateFieldValues = {}
  for (const field of template.fields) {
    if (!field.binding) {
      values[field.name] = ''
      continue
    }
    values[field.name] = resolveBinding(field.binding, facts) ?? ''
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
 * Validate required fields against the template. Missing/blank required values
 * fail issuance — never produce a malformed canonical artifact.
 */
export function validateFormValues(
  template: TemplateDefinition,
  values: TemplateFieldValues,
): FormValidationIssue[] {
  const issues: FormValidationIssue[] = []
  for (const field of template.fields) {
    if (!field.required) continue
    const value = (values[field.name] ?? '').trim()
    if (!value) {
      issues.push({
        field: field.name,
        label: field.label,
        message: `${field.label} is required.`,
      })
    }
  }
  return issues
}
