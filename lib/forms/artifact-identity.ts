import type {
  TemplateFieldValues,
  TemplateSectionValues,
} from './template-types'

function sortedRecord(value: Record<string, string>): Record<string, string> {
  return Object.fromEntries(
    Object.entries(value).sort(([left], [right]) => left.localeCompare(right)),
  )
}

/**
 * Stable, client-safe identity for the draft content that feeds composition.
 * This is not a security checksum; issued bytes retain their SHA-256 checksum.
 */
export function formContentFingerprint(
  fieldValues: TemplateFieldValues,
  sections: TemplateSectionValues,
): string {
  return JSON.stringify({
    fieldValues: sortedRecord(fieldValues),
    sections: sortedRecord(sections),
  })
}
