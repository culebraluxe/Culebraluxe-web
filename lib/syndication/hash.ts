import type { ListingSource } from './types'

// ---------------------------------------------------------------------------
// Root change fingerprint (V3 §2.1). When the source-of-truth facts that drive
// an off-site placement change, the saved pack is stale. We hash the root facts
// at prepare time and store it on the placement; a later comparison flags rows
// that need a regenerate. Hash is deterministic and cheap — it is NOT a security
// primitive.
// ---------------------------------------------------------------------------

/** Root fields whose change should invalidate non-site packs. */
function fingerprintParts(source: ListingSource): string[] {
  return [
    source.id,
    source.name ?? '',
    source.isPublished ? 'pub' : 'draft',
    String(source.listPrice ?? ''),
    source.bedrooms != null ? `b${source.bedrooms}` : '',
    source.bathrooms != null ? `ba${source.bathrooms}` : '',
    source.heroMediaId ?? '',
  ]
}

/** FNV-1a over the canonical root fingerprint. Stable across process restarts. */
export function computeListingSourceHash(source: ListingSource): string {
  const input = fingerprintParts(source).join('|')
  let hash = 0x811c9dc5
  for (let i = 0; i < input.length; i += 1) {
    hash ^= input.charCodeAt(i)
    hash = Math.imul(hash, 0x01000193)
  }
  return `sh_${(hash >>> 0).toString(16)}`
}

/** True when the current source no longer matches the hash stored on a placement. */
export function isSourceStale(source: ListingSource, storedHash: string | null): boolean {
  if (!storedHash) return false
  return computeListingSourceHash(source) !== storedHash
}
