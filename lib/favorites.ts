// Browser-local Favorites V1 storage contract (PX-22).
// Canonical ids/slugs/public names only; no sensitive data; dedupe by
// canonical property id. Stale/non-public entries are pruned by the caller
// against the live public property set before rendering. Backward compatible
// with the legacy array-of-id format, so previously saved hearts survive.
//
// Cross-component sync mirrors the compare seam (lib/compare.ts): every real
// persisted change dispatches FAVORITES_CHANGED_EVENT so independent hearts
// (property page, buyers showroom, home carousel) and the saved-properties
// page stay in agreement without shared state.

export type SavedPropertyEntry = {
  id: string
  slug: string
  name: string
}

const STORAGE_KEY = 'culebraluxe:saved-properties'

// Fired after every persisted change so independent client components stay in
// sync. Browser-only; never dispatched server-side.
export const FAVORITES_CHANGED_EVENT = 'culebraluxe:favorites-changed'

function dispatchFavoritesChanged() {
  if (typeof window === 'undefined') return
  window.dispatchEvent(new CustomEvent(FAVORITES_CHANGED_EVENT))
}

function isEntry(value: unknown): value is SavedPropertyEntry {
  return (
    Boolean(value) &&
    typeof (value as SavedPropertyEntry).id === 'string' &&
    typeof (value as SavedPropertyEntry).slug === 'string' &&
    typeof (value as SavedPropertyEntry).name === 'string'
  )
}

// Reads both the legacy array-of-ids format and the current
// array-of-{id,slug,name} format. Canonical ids only; no sensitive data.
export function readFavorites(): SavedPropertyEntry[] {
  if (typeof window === 'undefined') return []
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return parsed
      .map((entry): SavedPropertyEntry | null => {
        if (typeof entry === 'string') {
          return entry ? { id: entry, slug: '', name: '' } : null
        }
        return isEntry(entry) ? entry : null
      })
      .filter((entry): entry is SavedPropertyEntry => Boolean(entry))
  } catch {
    return []
  }
}

// Persists entries and notifies listeners. No-op writes never notify, so
// refresh/prune listeners converge instead of re-triggering themselves.
export function writeFavorites(entries: SavedPropertyEntry[]) {
  if (typeof window === 'undefined') return
  try {
    const next = JSON.stringify(entries)
    if (window.localStorage.getItem(STORAGE_KEY) === next) return
    window.localStorage.setItem(STORAGE_KEY, next)
    dispatchFavoritesChanged()
  } catch {
    // ignore storage failures (private mode, etc.)
  }
}

export function isFavorite(id: string): boolean {
  return readFavorites().some((entry) => entry.id === id)
}

// Returns true when the entry is now saved, false when removed. The reported
// state is re-read from storage after the write so it always agrees with what
// was actually persisted (a failed write reports false, never a phantom save).
export function toggleFavorite(entry: SavedPropertyEntry): boolean {
  const current = readFavorites()
  const exists = current.some((existing) => existing.id === entry.id)
  const next = exists
    ? current.filter((existing) => existing.id !== entry.id)
    : [...current, entry]
  writeFavorites(next)
  return readFavorites().some((existing) => existing.id === entry.id)
}

// Removes a single entry by canonical property id; returns true when an entry
// was actually removed. Persists and notifies listeners on a real removal.
export function removeFavorite(id: string): boolean {
  const current = readFavorites()
  const next = current.filter((existing) => existing.id !== id)
  if (next.length === current.length) return false
  writeFavorites(next)
  return true
}

// Removes stored entries that no longer correspond to a live public property
// (matched by canonical id or slug), persists the pruned list so delisted
// listings cannot permanently consume the saved set, and returns survivors.
export function pruneFavorites(
  validIds: string[],
  validSlugs: string[],
): SavedPropertyEntry[] {
  const ids = new Set(validIds)
  const slugs = new Set(validSlugs)
  const pruned = readFavorites().filter(
    (entry) => ids.has(entry.id) || slugs.has(entry.slug),
  )
  writeFavorites(pruned)
  return pruned
}
