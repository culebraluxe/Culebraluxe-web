import type { ListingSource, PlacementRow } from './types'
import { isSourceStale } from './hash'

// ---------------------------------------------------------------------------
// Listing lifecycle helpers (V3 §2.2 / §2.7). Pure and shared so the UI and
// tests agree about "off market" and "needs me" without hitting the DB.
// ---------------------------------------------------------------------------

export type NeedsFilter = 'all' | 'needs_me' | 'live' | 'expired'

const OFF_MARKET_STATUS = new Set(['sold', 'withdrawn', 'archived'])

/** True when the source listing is not currently being marketed. */
export function isOffMarket(source: Pick<ListingSource, 'isPublished' | 'status'>): boolean {
  const status = (source.status ?? '').toLowerCase()
  return !source.isPublished || OFF_MARKET_STATUS.has(status)
}

/**
 * A placement "needs me" when it is pending confirmation, expired, or its saved
 * root-fact hash no longer matches the current listing (stale pack). The site
 * channel is driven by is_published and never needs a manual refresh.
 */
export function placementNeedsMe(
  source: ListingSource | null,
  row: PlacementRow,
): boolean {
  if (row.status === 'expired') return true
  if (row.status === 'pending_manual') return true
  if (row.channel !== 'culebraluxe' && row.sourceHash && source && isSourceStale(source, row.sourceHash)) {
    return true
  }
  return false
}

export function matchesNeedsFilter(
  filter: NeedsFilter,
  source: ListingSource | null,
  row: PlacementRow,
): boolean {
  switch (filter) {
    case 'all':
      return true
    case 'needs_me':
      return placementNeedsMe(source, row)
    case 'live':
      return row.status === 'live'
    case 'expired':
      return row.status === 'expired'
  }
}
