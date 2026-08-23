// Canonical service-intent vocabulary surfaced by the public Services page.
//
// This is the single allow-list for `/contact?service=...`. The public
// Services page CTA hrefs use these exact keys, and the contact intake only
// ever persists a value that survives `normalizeServiceKey`, so arbitrary or
// forged query-string values are never written as submission metadata.
export const SUPPORTED_SERVICE_KEYS = [
  'market-analysis',
  'property-evaluation',
  'comparable-research',
  'land-survey',
  'appraisal',
  'title-research',
  'consultation',
  'property-marketing',
] as const

export type SupportedServiceKey = (typeof SUPPORTED_SERVICE_KEYS)[number]

/**
 * Normalize an untrusted `service` query value against the supported allow-list.
 * Returns `undefined` for a missing, empty, or unrecognized value so an
 * unsupported intent is simply dropped rather than persisted.
 */
export function normalizeServiceKey(
  value: string | undefined,
): SupportedServiceKey | undefined {
  if (!value) return undefined
  const trimmed = value.trim()
  return (SUPPORTED_SERVICE_KEYS as readonly string[]).includes(trimmed)
    ? (trimmed as SupportedServiceKey)
    : undefined
}
