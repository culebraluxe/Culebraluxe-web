import type {
  TemplateDefinition,
  TemplateFieldValues,
} from './template-types'

/** Deterministic USD formatting for money fields. */
export function formatMoney(value: string): string {
  const digits = value.replace(/[^0-9.]/g, '')
  if (!digits) return value.trim()
  const [whole, decimal] = digits.split('.')
  const grouped = whole.replace(/\B(?=(\d{3})+(?!\d))/g, ',')
  return decimal ? `$${grouped}.${decimal}` : `$${grouped}`
}

const MONTHS = [
  'January', 'February', 'March', 'April', 'May', 'June',
  'July', 'August', 'September', 'October', 'November', 'December',
]

/** Deterministic ISO (YYYY-MM-DD) → 'Month D, YYYY' date formatting. */
export function formatDate(value: string): string {
  const match = /^(\d{4})-(\d{2})-(\d{2})/.exec(value.trim())
  if (!match) return value.trim()
  const year = Number(match[1])
  const month = Number(match[2])
  const day = Number(match[3])
  if (month < 1 || month > 12 || day < 1 || day > 31) return value.trim()
  return `${MONTHS[month - 1]} ${day}, ${year}`
}

/** Format one field value for rendering (money/date aware). */
export function formatFieldValue(
  field: { type: string; name: string },
  raw: string,
): string {
  if (field.type === 'money' && raw.trim()) return formatMoney(raw)
  if (field.type === 'date' && raw.trim()) return formatDate(raw)
  return raw
}

/**
 * DOC-08 — interpolate a section's default segments: literal text plus the
 * declared `<value field="X"/>` substitutions.
 */
export function interpolateSectionText(
  section: {
    segments: readonly { kind: 'text' | 'value'; text?: string; field?: string }[]
  },
  values: TemplateFieldValues,
  format: (field: { type: string; name: string }, raw: string) => string,
  fields: readonly { type: string; name: string }[] = [],
): string {
  let out = ''
  for (const segment of section.segments ?? []) {
    if (segment.kind === 'text') {
      out += segment.text ?? ''
    } else {
      const field = fields.find((f) => f.name === segment.field)
      const raw = (values[segment.field ?? ''] ?? '').trim()
      out += field ? format(field, raw) : raw
    }
  }
  return out.replace(/\s+/g, ' ').trim()
}

/**
 * Plain-text Word-like body from XML <section> copy with <value> filled in.
 * No XML tags. Used as the starting text of the freeform editor.
 */
export function documentBodyText(
  template: Pick<TemplateDefinition, 'sections' | 'fields'>,
  values: TemplateFieldValues,
): string {
  return template.sections
    .map((section) => {
      const text = interpolateSectionText(
        section,
        values,
        formatFieldValue,
        template.fields,
      )
      return text ? `${section.label}\n${text}` : section.label
    })
    .join('\n\n')
}
