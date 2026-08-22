'use client'

import { useEffect, useMemo, useState } from 'react'
import { useRouter } from 'next/navigation'
import { Bell, Bookmark, BookmarkCheck, X } from 'lucide-react'

import type { PropertySummary } from '@/db/properties'
import {
  SAVED_SEARCHES_CHANGED_EVENT,
  markSavedSearchViewed,
  newMatchIds,
  readSavedSearches,
  removeSavedSearch,
  saveSearch,
} from '@/lib/saved-searches'
import type { SavedSearch } from '@/lib/saved-searches'
import {
  describeSearchFilters,
  matchesSearchFilters,
  searchFiltersToKey,
  searchFiltersToQuery,
} from '@/lib/search-contract'
import type { SearchFilters } from '@/lib/search-contract'

type SavedSearchesPanelProps = {
  /** The full live active inventory (server-fetched), used to count matches. */
  inventory: PropertySummary[]
  /** The current control state on the /buyers page (what Save would capture). */
  currentFilters: SearchFilters
}

type SearchView = {
  search: SavedSearch
  matchIds: string[]
  newIds: string[]
}

/**
 * PX-23 Saved Searches + Alerts V1 — browser-local. Saves the current /buyers
 * filter state, re-runs each saved search against the live inventory on every
 * render, and shows an honest "N new" alert for matches the search has not
 * been viewed against yet. Applying a search navigates to the server-filtered
 * /buyers URL (the server stays authoritative) and marks it viewed.
 */
export function SavedSearchesPanel({
  inventory,
  currentFilters,
}: SavedSearchesPanelProps) {
  const router = useRouter()
  const [searches, setSearches] = useState<SavedSearch[]>([])

  useEffect(() => {
    const sync = () => setSearches(readSavedSearches())
    sync()
    window.addEventListener(SAVED_SEARCHES_CHANGED_EVENT, sync)
    return () => window.removeEventListener(SAVED_SEARCHES_CHANGED_EVENT, sync)
  }, [])

  const views = useMemo<SearchView[]>(
    () =>
      searches.map((search) => {
        const matchIds = inventory
          .filter((property) => matchesSearchFilters(property, search.filters))
          .map((property) => property.id)
        return { search, matchIds, newIds: newMatchIds(search, matchIds) }
      }),
    [searches, inventory],
  )

  const currentMatchIds = useMemo(
    () =>
      inventory
        .filter((property) => matchesSearchFilters(property, currentFilters))
        .map((property) => property.id),
    [inventory, currentFilters],
  )

  const currentKey = searchFiltersToKey(currentFilters)
  const currentIsSaved = views.some(
    (view) => searchFiltersToKey(view.search.filters) === currentKey,
  )
  const totalNew = views.reduce((sum, view) => sum + view.newIds.length, 0)

  const handleSave = () => {
    saveSearch({
      name: describeSearchFilters(currentFilters),
      filters: currentFilters,
      initialMatchIds: currentMatchIds,
    })
  }

  const handleApply = (view: SearchView) => {
    // Marking viewed first clears the alert for this search; the navigation
    // then lands on the server-filtered results for the same filters.
    markSavedSearchViewed(view.search.id, view.matchIds)
    const qs = searchFiltersToQuery(view.search.filters)
    router.push(qs ? `/buyers?${qs}` : '/buyers')
  }

  const handleRemove = (id: string) => {
    removeSavedSearch(id)
  }

  return (
    <div className="mb-12 flex flex-col gap-4 border border-border bg-muted/30 px-5 py-4 md:flex-row md:items-center md:justify-between">
      <button
        type="button"
        onClick={handleSave}
        aria-pressed={currentIsSaved}
        className="flex h-12 min-w-0 items-center gap-3 border border-border bg-background px-5 text-xs font-light uppercase tracking-[0.2em] text-foreground transition-colors duration-300 hover:border-foreground"
      >
        {currentIsSaved ? (
          <BookmarkCheck
            className="h-4 w-4 flex-none text-accent"
            aria-hidden
            strokeWidth={1.5}
          />
        ) : (
          <Bookmark
            className="h-4 w-4 flex-none text-muted-foreground"
            aria-hidden
            strokeWidth={1.5}
          />
        )}
        <span className="truncate">
          {currentIsSaved ? 'Search saved' : 'Save this search'}
        </span>
      </button>

      {views.length > 0 ? (
        <div
          className="flex flex-wrap items-center gap-2"
          aria-label="Saved searches"
        >
          {totalNew > 0 && (
            <span className="inline-flex h-6 items-center gap-1.5 bg-accent/10 px-2.5 text-[10px] font-light uppercase tracking-[0.16em] text-accent">
              <Bell
                className="h-3 w-3"
                aria-hidden
                strokeWidth={1.5}
              />
              {totalNew} new across {views.length}
            </span>
          )}

          {views.map((view) => (
            <span
              key={view.search.id}
              className="inline-flex items-stretch border border-border bg-background"
            >
              <button
                type="button"
                onClick={() => handleApply(view)}
                title="Apply this saved search"
                className="group flex h-12 items-center gap-2 pl-4 pr-2 text-left"
              >
                <span className="max-w-56 truncate text-xs font-light tracking-wide text-foreground/90 group-hover:text-foreground">
                  {view.search.name}
                </span>
                <span
                  aria-hidden
                  className="text-[10px] font-light uppercase tracking-[0.14em] text-muted-foreground"
                >
                  {view.matchIds.length}
                </span>
                {view.newIds.length > 0 && (
                  <span className="text-[10px] font-light uppercase tracking-[0.14em] text-accent">
                    +{view.newIds.length} new
                  </span>
                )}
              </button>

              <button
                type="button"
                onClick={() => handleRemove(view.search.id)}
                aria-label={`Remove saved search ${view.search.name}`}
                className="group flex h-12 w-10 flex-none items-center justify-center text-muted-foreground transition-colors hover:text-foreground"
              >
                <X
                  className="h-3.5 w-3.5"
                  aria-hidden
                  strokeWidth={1.5}
                />
              </button>
            </span>
          ))}
        </div>
      ) : (
        <p className="text-xs font-light leading-relaxed text-muted-foreground">
          Save a search to get an alert when new matching properties appear.
        </p>
      )}
    </div>
  )
}
