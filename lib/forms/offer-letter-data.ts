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
}

export function emptyDealFacts(): DealFactsForForm {
  return {
    clientName: null,
    propertyLabel: null,
    offerAmount: null,
    financingType: null,
    closingDate: null,
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
    default:
      return null
  }
}

/**
 * Prefill a field-values map for a template from canonical facts. Bound fields
 * adopt the canonical value; unbound fields start blank (user-entered).
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
  return values
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
