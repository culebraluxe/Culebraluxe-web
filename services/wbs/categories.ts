/**
 * WBS category catalog — the fixed dimension that groups work items by the
 * major functional areas of the business (mirrors the site's top sections).
 *
 * Deliberately plural where a domain is a collection (Properties) and includes
 * a Management catch-all for follow-ups that don't fit a clean functional area.
 */
export const WBS_CATEGORIES = [
  { id: 'clients', label: 'Clients' },
  { id: 'contracts', label: 'Contracts' },
  { id: 'properties', label: 'Properties' },
  { id: 'media', label: 'Media' },
  { id: 'marketing', label: 'Marketing' },
  { id: 'accounting', label: 'Accounting' },
  { id: 'management', label: 'Management' },
] as const

export type WbsCategoryId = (typeof WBS_CATEGORIES)[number]['id']
export type WbsCategory = (typeof WBS_CATEGORIES)[number]

export const MANAGEMENT_CATEGORY_ID = 'management' as const

const CATEGORY_IDS: ReadonlySet<string> = new Set(WBS_CATEGORIES.map((c) => c.id))

export function isWbsCategory(value: string): value is WbsCategoryId {
  return CATEGORY_IDS.has(value)
}

export function wbsCategoryLabel(id: WbsCategoryId): string {
  return WBS_CATEGORIES.find((c) => c.id === id)?.label ?? id
}
