// Browser-local Compare Properties V1 storage contract.
// Canonical ids/slugs only; no sensitive data; bounded to 3 entries; dedupe
// by canonical property id. Stale/non-public entries are filtered by the
// caller against the live public property set before rendering.

export type CompareEntry = {
  id: string
  slug: string
  name: string
}

const STORAGE_KEY = 'culebraluxe:compare-properties'
export const COMPARE_MAX = 3

export function readCompare(): CompareEntry[] {
  if (typeof window === 'undefined') return []
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return parsed.filter(
      (entry): entry is CompareEntry =>
        Boolean(entry) &&
        typeof (entry as CompareEntry).id === 'string' &&
        typeof (entry as CompareEntry).slug === 'string' &&
        typeof (entry as CompareEntry).name === 'string',
    )
  } catch {
    return []
  }
}

export function writeCompare(entries: CompareEntry[]) {
  try {
    window.localStorage.setItem(
      STORAGE_KEY,
      JSON.stringify(entries.slice(0, COMPARE_MAX)),
    )
  } catch {
    // ignore storage failures (private mode, etc.)
  }
}

// Removes stored entries whose slug is no longer in the live public set,
// persists the pruned list (so stale entries cannot consume one of the three
// slots), and returns the surviving entries.
export function pruneCompare(validSlugs: string[]): CompareEntry[] {
  const valid = new Set(validSlugs)
  const pruned = readCompare().filter((entry) => valid.has(entry.slug))
  writeCompare(pruned)
  return pruned
}

// Returns true when the entry is now selected, false when removed.
export function toggleCompare(entry: CompareEntry): boolean {
  const current = readCompare()
  const exists = current.some((existing) => existing.id === entry.id)
  const next = exists
    ? current.filter((existing) => existing.id !== entry.id)
    : [...current, entry].slice(0, COMPARE_MAX)
  writeCompare(next)
  return !exists
}
