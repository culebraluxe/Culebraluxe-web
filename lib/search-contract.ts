// Canonical buyers search contract (PX-23). The search surface is the
// `/buyers` inventory page; its controls are category, free-text q, maxPrice,
// beds, view, and sort. The server (`db/properties.ts` `getFilteredProperties`)
// is the AUTHORITATIVE filter for what renders — this module is the pure
// client-side mirror used only by saved-search bookkeeping:
//
//   - a canonical signature for dedupe (`searchFiltersToKey`),
//   - the apply URL (`searchFiltersToQuery`), which reproduces exactly what
//     the showroom pushes into the URL, and
//   - the alert matcher (`matchesSearchFilters`), which mirrors the server SQL
//     contract so "N new matches" is counted over the live inventory without a
//     server round trip per saved search.
//
// The matcher deliberately ignores `sort` (ordering never changes which
// properties match) and keeps the same field set the server filters on:
// category, price, beds, view, free-text. PX-24 owns the next evolution of
// this contract; PX-23 consumes it as-is.

import type { PropertySummary } from '@/db/properties'
import { isLand } from '@/lib/property'

export type SearchCategory = 'all' | 'homes' | 'land'

export type SearchSort =
  | 'featured'
  | 'price-high'
  | 'price-low'
  | 'name'

// Control/URL-shaped filters: maxPrice and beds are the raw select strings
// ('' = any), exactly as the showroom holds them and as the URL carries them.
export type SearchFilters = {
  category: SearchCategory
  q: string
  maxPrice: string
  beds: string
  view: string
  sort: SearchSort
}

export const DEFAULT_SEARCH_FILTERS: SearchFilters = {
  category: 'all',
  q: '',
  maxPrice: '',
  beds: '',
  view: '',
  sort: 'featured',
}

// Canonical, order-independent signature of a filter set. Field order is
// fixed, free-text is trimmed/lowercased, the view is lowercased (the server
// lowercases the view for matching), and numeric strings are trimmed — so
// logically identical searches always dedupe to the same key regardless of
// how the caller constructed the object.
export function searchFiltersToKey(filters: SearchFilters): string {
  return [
    filters.category,
    filters.q.trim().toLowerCase(),
    filters.maxPrice.trim(),
    filters.beds.trim(),
    filters.view.trim().toLowerCase(),
    filters.sort,
  ].join('|')
}

// Reproduces the showroom's URL push contract exactly: defaults are omitted,
// free-text is trimmed, and an empty result means "no query string".
export function searchFiltersToQuery(filters: SearchFilters): string {
  const params = new URLSearchParams()
  if (filters.category !== 'all') params.set('category', filters.category)
  if (filters.q.trim()) params.set('q', filters.q.trim())
  if (filters.maxPrice.trim()) params.set('maxPrice', filters.maxPrice.trim())
  if (filters.beds.trim()) params.set('beds', filters.beds.trim())
  if (filters.view.trim()) params.set('view', filters.view.trim())
  if (filters.sort !== 'featured') params.set('sort', filters.sort)
  return params.toString()
}

// Human label for a saved search, derived only from the non-default filters.
// Ordering is not a property of the match set, so sort never appears.
export function describeSearchFilters(filters: SearchFilters): string {
  const parts: string[] = []

  if (filters.category === 'homes') parts.push('Homes & Villas')
  else if (filters.category === 'land') parts.push('Land')

  if (filters.maxPrice.trim()) {
    const price = Number(filters.maxPrice)
    if (Number.isFinite(price) && price > 0) {
      parts.push(`up to $${price.toLocaleString('en-US')}`)
    }
  }

  if (filters.beds.trim()) {
    const beds = Number(filters.beds)
    if (Number.isFinite(beds) && beds > 0) parts.push(`${beds}+ beds`)
  }

  if (filters.view.trim()) parts.push(`${filters.view.trim()} view`)

  if (filters.q.trim()) parts.push(`\u201c${filters.q.trim()}\u201d`)

  return parts.length > 0 ? parts.join(' · ') : 'All properties'
}

// Pure client-side mirror of the server filter SQL (db/properties.ts
// getFilteredProperties): category via property type, price cap, beds floor
// (never land), view membership, and the free-text haystack (name, location,
// city, neighborhood, property type, view words). Used only to count matches
// for saved-search alerts over the already-fetched live inventory; the server
// remains authoritative for what actually renders.
export function matchesSearchFilters(
  property: PropertySummary,
  filters: SearchFilters,
): boolean {
  if (filters.category === 'land' && !isLand(property.propertyType)) {
    return false
  }
  if (filters.category === 'homes' && isLand(property.propertyType)) {
    return false
  }

  const maxPrice = Number(filters.maxPrice)
  if (
    filters.maxPrice.trim() !== '' &&
    Number.isFinite(maxPrice) &&
    (property.listPrice == null || property.listPrice > maxPrice)
  ) {
    return false
  }

  const beds = Number(filters.beds)
  if (
    filters.beds.trim() !== '' &&
    Number.isFinite(beds) &&
    (isLand(property.propertyType) ||
      property.bedrooms == null ||
      property.bedrooms < beds)
  ) {
    return false
  }

  const view = filters.view.trim()
  if (
    view &&
    !property.views.some(
      (candidate) => candidate.toLowerCase() === view.toLowerCase(),
    )
  ) {
    return false
  }

  const q = filters.q.trim().toLowerCase()
  if (q) {
    const haystack = [
      property.name,
      property.location,
      property.city,
      property.neighborhood,
      property.propertyType,
      ...property.views,
    ]
      .filter(Boolean)
      .join(' ')
      .toLowerCase()

    if (!haystack.includes(q)) return false
  }

  return true
}
