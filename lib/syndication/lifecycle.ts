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

// ---------------------------------------------------------------------------
// Root-fact snapshot (Task 2). The placement stores the source facts at prepare
// time; the stale banner lists exactly what changed. Pure so the UI and tests
// share it.
// ---------------------------------------------------------------------------

export type SourceSnapshot = {
  name?: string
  price?: number | null
  beds?: number | null
  baths?: number | null
  published?: boolean
}

export function makeSnapshot(source: ListingSource): SourceSnapshot {
  return {
    name: source.name,
    price: source.listPrice,
    beds: source.bedrooms,
    baths: source.bathrooms,
    published: source.isPublished,
  }
}

function fmtMoney(value: number | null | undefined): string {
  if (value == null || !Number.isFinite(value)) return 'n/a'
  return new Intl.NumberFormat('en-US', { style: 'currency', currency: 'USD', maximumFractionDigits: 0 }).format(value)
}

/** Human-readable list of what changed between the saved snapshot and now. */
export function diffSnapshot(source: ListingSource, snapshot: SourceSnapshot | null | undefined): string[] {
  if (!snapshot) return []
  const diffs: string[] = []
  if (typeof snapshot.name === 'string' && snapshot.name !== source.name) {
    diffs.push(`Name: ${snapshot.name} → ${source.name}`)
  }
  if ('price' in snapshot && (snapshot.price ?? null) !== (source.listPrice ?? null)) {
    diffs.push(`Price: ${fmtMoney(snapshot.price)} → ${fmtMoney(source.listPrice)}`)
  }
  if ('beds' in snapshot && (snapshot.beds ?? null) !== (source.bedrooms ?? null)) {
    diffs.push(`Beds: ${snapshot.beds ?? '—'} → ${source.bedrooms ?? '—'}`)
  }
  if ('baths' in snapshot && (snapshot.baths ?? null) !== (source.bathrooms ?? null)) {
    diffs.push(`Baths: ${snapshot.baths ?? '—'} → ${source.bathrooms ?? '—'}`)
  }
  if (typeof snapshot.published === 'boolean' && snapshot.published !== source.isPublished) {
    diffs.push(source.isPublished ? 'Now public on the site' : 'Now off the public site')
  }
  return diffs
}

// ---------------------------------------------------------------------------
// Launch completeness score (Task 4). 0–100 on the Launch panel. Clasificados and
// sightings do not score.
// ---------------------------------------------------------------------------

type ScoreSpec = { weight: number; met: boolean; label: string }

export function launchScore(
  source: ListingSource,
  rows: PlacementRow[],
): { score: number; missing: string[] } {
  const stellar = rows.find((r) => r.channel === 'stellar_mls')
  const stellarConfirmed = !!stellar && (stellar.status === 'live' || !!stellar.externalUrl || !!stellar.externalId)
  const facebook = rows.find((r) => r.channel === 'facebook_marketplace')
  const facebookConfirmed = !!facebook && !!facebook.externalUrl

  const spec: ScoreSpec[] = [
    { weight: 20, met: !!source.isPublished, label: 'Published on the site' },
    { weight: 15, met: !!source.heroMediaId, label: 'Hero photo set' },
    { weight: 15, met: (source.imageCount ?? 0) >= 5, label: 'At least 5 photos' },
    { weight: 15, met: source.listPrice != null, label: 'Asking price set' },
    { weight: 10, met: !!source.city, label: 'City on the listing' },
    { weight: 15, met: stellarConfirmed, label: 'Stellar confirmed (MLS#)' },
    { weight: 10, met: facebookConfirmed, label: 'Facebook URL confirmed' },
  ]
  const score = spec.filter((s) => s.met).reduce((sum, s) => sum + s.weight, 0)
  return {
    score,
    missing: spec.filter((s) => !s.met).map((s) => s.label),
  }
}
