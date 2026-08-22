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

// Fired after every persisted change so independent client components (the
// per-card toggles and the comparison table) stay in sync without sharing
// state. Browser-only; never dispatched server-side.
export const COMPARE_CHANGED_EVENT = 'culebraluxe:compare-changed'

function dispatchCompareChanged() {
  if (typeof window === 'undefined') return
  window.dispatchEvent(new CustomEvent(COMPARE_CHANGED_EVENT))
}

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
    const next = JSON.stringify(entries.slice(0, COMPARE_MAX))
    // No-op writes never notify: listeners (prune/refresh loops) converge
    // instead of re-triggering themselves.
    if (window.localStorage.getItem(STORAGE_KEY) === next) return
    window.localStorage.setItem(STORAGE_KEY, next)
    dispatchCompareChanged()
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

// Removes a single entry by canonical property id; returns true when an entry
// was actually removed. Persists and notifies listeners on a real removal.
export function removeCompare(id: string): boolean {
  const current = readCompare()
  const next = current.filter((existing) => existing.id !== id)
  if (next.length === current.length) return false
  writeCompare(next)
  return true
}

// Returns true when the entry is now selected, false when removed.
// Adding past COMPARE_MAX is rejected rather than silently dropping the
// newest entry, and the reported state is re-read from storage after the
// write so it always agrees with what was actually persisted (a rejected or
// failed write reports false, never a phantom selection).
export function toggleCompare(entry: CompareEntry): boolean {
  const current = readCompare()
  const exists = current.some((existing) => existing.id === entry.id)
  if (exists) {
    writeCompare(current.filter((existing) => existing.id !== entry.id))
    return readCompare().some((existing) => existing.id === entry.id)
  }
  if (current.length >= COMPARE_MAX) return false
  writeCompare([...current, entry])
  return readCompare().some((existing) => existing.id === entry.id)
}
