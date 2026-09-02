import type { PhotoManifestItem } from './types'

// ---------------------------------------------------------------------------
// Photos are first-class syndication data. We NEVER send the listing HTML page
// URL as an image to Meta/Clasificados. Real images come from property_media ⋈
// media and are served by the app's existing `/api/media/{id}` route. This
// module builds the absolute public URLs and the ordered manifest (hero first,
// capped at 25) that adapters embed into their payloads.
// ---------------------------------------------------------------------------

export const DEFAULT_PUBLIC_ORIGIN = 'https://culebraluxe.com'

/** Public image path uses the existing in-app media route, not a second CDN. */
export function mediaUrl(mediaId: string, origin: string = DEFAULT_PUBLIC_ORIGIN): string {
  return `${origin.replace(/\/$/, '')}/api/media/${encodeURIComponent(mediaId)}`
}

export type PhotoDbRow = {
  media_id: string
  role: string | null
  sort_order: number | null
  width?: number | null
  height?: number | null
  mime_type?: string | null
}

/**
 * Pure mapping of property_media ⋈ media rows into an ordered photo manifest.
 * Hero rows sort first, then by sort_order, then media_id for determinism.
 * Capped at 25 so we never push an unbounded image list to a third party.
 */
export function buildPhotoManifest(
  rows: PhotoDbRow[],
  origin: string = DEFAULT_PUBLIC_ORIGIN,
): PhotoManifestItem[] {
  const ranked = [...rows]
    .sort((a, b) => {
      const roleA = a.role === 'hero' ? 0 : 1
      const roleB = b.role === 'hero' ? 0 : 1
      if (roleA !== roleB) return roleA - roleB
      const soA = a.sort_order ?? 0
      const soB = b.sort_order ?? 0
      if (soA !== soB) return soA - soB
      return a.media_id.localeCompare(b.media_id)
    })
    .slice(0, 25)

  return ranked.map((row) => ({
    mediaId: row.media_id,
    url: mediaUrl(row.media_id, origin),
    role: row.role === 'hero' ? ('hero' as const) : ('gallery' as const),
    sortOrder: row.sort_order ?? 0,
    width: row.width ?? undefined,
    height: row.height ?? undefined,
    contentType: row.mime_type ?? undefined,
  }))
}

export { mediaUrl as buildPhotoUrl }
