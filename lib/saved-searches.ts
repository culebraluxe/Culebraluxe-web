// Browser-local Saved Searches + Alerts V1 storage contract (PX-23).
// Canonical search filters only; no sensitive data; deduped by the canonical
// filter signature (searchFiltersToKey), so re-saving an identical search
// refreshes the existing entry instead of duplicating it. Alerts are an
// honest "new matches since you last viewed" diff: each search remembers the
// canonical property ids seen at its last view, and the caller counts current
// matches not yet in that set (lib/search-contract.matchesSearchFilters).
//
// Cross-component sync mirrors the favorites/compare seams: every real
// persisted change dispatches SAVED_SEARCHES_CHANGED_EVENT so the save control
// and the saved-searches panel stay in agreement without shared state.

import {
  searchFiltersToKey,
} from '@/lib/search-contract'
import type { SearchFilters } from '@/lib/search-contract'

export type SavedSearch = {
  id: string
  name: string
  filters: SearchFilters
  createdAt: string
  lastCheckedAt: string | null
  // Canonical property ids seen at the last view; alerts = current matches
  // minus this set. Never an authority on inventory — only a seen-marker.
  lastMatchIds: string[]
}

export type SaveSearchInput = {
  name: string
  filters: SearchFilters
  /** Canonical ids of the properties matching `filters` right now. */
  initialMatchIds?: string[]
}

const STORAGE_KEY = 'culebraluxe:saved-searches'

// Fired after every persisted change so independent client components stay in
// sync. Browser-only; never dispatched server-side.
export const SAVED_SEARCHES_CHANGED_EVENT = 'culebraluxe:saved-searches-changed'

function dispatchSavedSearchesChanged() {
  if (typeof window === 'undefined') return
  window.dispatchEvent(new CustomEvent(SAVED_SEARCHES_CHANGED_EVENT))
}

function isSearchFilters(value: unknown): value is SearchFilters {
  if (!value || typeof value !== 'object') return false
  const filters = value as Record<string, unknown>
  const category = filters.category
  return (
    (category === 'all' || category === 'homes' || category === 'land') &&
    typeof filters.q === 'string' &&
    typeof filters.maxPrice === 'string' &&
    typeof filters.beds === 'string' &&
    typeof filters.view === 'string' &&
    (filters.sort === 'featured' ||
      filters.sort === 'price-high' ||
      filters.sort === 'price-low' ||
      filters.sort === 'name')
  )
}

function isSavedSearch(value: unknown): value is SavedSearch {
  if (!value || typeof value !== 'object') return false
  const entry = value as Record<string, unknown>
  return (
    typeof entry.id === 'string' &&
    typeof entry.name === 'string' &&
    isSearchFilters(entry.filters) &&
    typeof entry.createdAt === 'string' &&
    (entry.lastCheckedAt === null || typeof entry.lastCheckedAt === 'string') &&
    Array.isArray(entry.lastMatchIds) &&
    entry.lastMatchIds.every((id) => typeof id === 'string')
  )
}

// Monotonic-ish, collision-resistant-enough id generator that needs no crypto
// (safe in every browser and in the Node test harness).
let idCounter = 0
function createSavedSearchId(): string {
  idCounter = (idCounter + 1) % 1000
  return `ss-${Date.now().toString(36)}-${idCounter.toString(36)}${Math.random()
    .toString(36)
    .slice(2, 8)}`
}

export function readSavedSearches(): SavedSearch[] {
  if (typeof window === 'undefined') return []
  try {
    const raw = window.localStorage.getItem(STORAGE_KEY)
    if (!raw) return []
    const parsed = JSON.parse(raw) as unknown
    if (!Array.isArray(parsed)) return []
    return parsed.filter(isSavedSearch)
  } catch {
    return []
  }
}

// Persists entries and notifies listeners. No-op writes never notify, so
// refresh/prune listeners converge instead of re-triggering themselves.
export function writeSavedSearches(searches: SavedSearch[]) {
  if (typeof window === 'undefined') return
  try {
    const next = JSON.stringify(searches)
    if (window.localStorage.getItem(STORAGE_KEY) === next) return
    window.localStorage.setItem(STORAGE_KEY, next)
    dispatchSavedSearchesChanged()
  } catch {
    // ignore storage failures (private mode, etc.)
  }
}

// Saves (or refreshes) a search. Saving an identical filter signature updates
// the existing entry — name refreshed, seen-state reset to the current match
// set so a re-save means "watching from now" — instead of creating a
// duplicate. The reported `created` flag is re-read from storage after the
// write so it always agrees with what was actually persisted (a failed or
// server-side write reports created:false, never a phantom save).
export function saveSearch(input: SaveSearchInput): {
  id: string
  created: boolean
} {
  const current = readSavedSearches()
  const key = searchFiltersToKey(input.filters)
  const existing = current.find(
    (entry) => searchFiltersToKey(entry.filters) === key,
  )
  const now = new Date().toISOString()
  let id: string
  let wasNew = false

  if (existing) {
    id = existing.id
    writeSavedSearches(
      current.map((entry) =>
        entry.id === existing.id
          ? {
              ...entry,
              name: input.name,
              lastCheckedAt: now,
              lastMatchIds: input.initialMatchIds ?? entry.lastMatchIds,
            }
          : entry,
      ),
    )
  } else {
    id = createSavedSearchId()
    wasNew = true
    const entry: SavedSearch = {
      id,
      name: input.name,
      filters: input.filters,
      createdAt: now,
      lastCheckedAt: now,
      lastMatchIds: input.initialMatchIds ?? [],
    }
    writeSavedSearches([...current, entry])
  }

  const persisted = readSavedSearches().some((entry) => entry.id === id)
  return { id, created: persisted && wasNew }
}

// Removes a single search by id; returns true when an entry was actually
// removed. Persists and notifies listeners on a real removal.
export function removeSavedSearch(id: string): boolean {
  const current = readSavedSearches()
  const next = current.filter((entry) => entry.id !== id)
  if (next.length === current.length) return false
  writeSavedSearches(next)
  return true
}

// Records that a search was viewed: the caller passes the canonical ids of
// the CURRENT matches, which become the new seen set, so the next alert diff
// starts from here. Returns true when the entry was found and updated.
export function markSavedSearchViewed(id: string, matchIds: string[]): boolean {
  const current = readSavedSearches()
  const existing = current.find((entry) => entry.id === id)
  if (!existing) return false
  const now = new Date().toISOString()
  writeSavedSearches(
    current.map((entry) =>
      entry.id === id
        ? { ...entry, lastMatchIds: matchIds, lastCheckedAt: now }
        : entry,
    ),
  )
  return true
}

// Pure diff: the subset of `matchIds` the search has not yet been viewed
// against. This is the alert — "N new matches since you last viewed".
export function newMatchIds(
  search: SavedSearch,
  matchIds: string[],
): string[] {
  const seen = new Set(search.lastMatchIds)
  return matchIds.filter((id) => !seen.has(id))
}
