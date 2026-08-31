// ---------------------------------------------------------------------------
// One declarative visibility primitive for XML templates.
//
//   when="financing:Cash"
//   when="financing:Bank,Blend"
//
// No expressions, no operators, no XPath. A field or section is visible when:
//   - it has no when attribute, or
//   - the named field equals one of the listed values (case-insensitive), or
//   - the named field equals "Show All" (QA review of the full superset).
//
// "Show All" is never a live transaction type. CRM-26 must not promote it.
// ---------------------------------------------------------------------------

export const SHOW_ALL_VALUE = 'Show All'

export type TemplateWhen = {
  field: string
  values: readonly string[]
}

export function parseWhenAttr(raw: string | undefined): TemplateWhen | null {
  if (!raw || !raw.trim()) return null
  const trimmed = raw.trim()
  const colon = trimmed.indexOf(':')
  if (colon <= 0) {
    throw new Error(
      `when must be "field:Value" or "field:Value,Value" (got "${raw}").`,
    )
  }
  const field = trimmed.slice(0, colon).trim()
  const values = trimmed
    .slice(colon + 1)
    .split(',')
    .map((value) => value.trim())
    .filter(Boolean)
  if (!field || values.length === 0) {
    throw new Error(`when must name a field and at least one value (got "${raw}").`)
  }
  return { field, values }
}

export function isShowAllValue(value: string | undefined | null): boolean {
  return (value ?? '').trim().toLowerCase() === SHOW_ALL_VALUE.toLowerCase()
}

export function isWhenSatisfied(
  when: TemplateWhen | null | undefined,
  values: Readonly<Record<string, string>>,
): boolean {
  if (!when) return true
  const actual = (values[when.field] ?? '').trim()
  if (!actual) return false
  if (isShowAllValue(actual)) return true
  return when.values.some(
    (allowed) => allowed.toLowerCase() === actual.toLowerCase(),
  )
}

export function visibleTemplateFields<
  T extends { name: string; when?: TemplateWhen | null },
>(fields: readonly T[], values: Readonly<Record<string, string>>): T[] {
  return fields.filter((field) => isWhenSatisfied(field.when, values))
}

export function visibleTemplateSections<
  T extends { when?: TemplateWhen | null },
>(sections: readonly T[], values: Readonly<Record<string, string>>): T[] {
  return sections.filter((section) => isWhenSatisfied(section.when, values))
}
