// ---------------------------------------------------------------------------
// Marketing — media download helpers (Block A / story 1).
// Photos live in property_media ⋈ media.file_data. The workbench never ships
// bytes to the browser; the download route queries them itself. These pure
// helpers are shared with the route and the tests (cap 25).
// ---------------------------------------------------------------------------

export const PHOTO_CAP = 25

export function mediaPublicPath(mediaId: string): string {
  return `/api/media/${encodeURIComponent(mediaId)}`
}

export function mediaPublicUrl(mediaId: string, origin = 'https://culebraluxe.com'): string {
  return `${origin.replace(/\/$/, '')}${mediaPublicPath(mediaId)}`
}

/** A safe filename for the download, e.g. `playa-flamenco-villa-photos` (.zip / .txt). */
export function photoDownloadBase(slug: string | null, name: string): string {
  const base = (slug ?? name)
    .toLowerCase()
    .replace(/[^a-z0-9]+/g, '-')
    .replace(/^-+|-+$/g, '')
  return `${base || 'listing'}-photos`
}

/** Ordered public URL list for a set of photo media rows (cap 25). */
export function photoUrlList(rows: Array<{ media_id: string }>): string[] {
  return rows.slice(0, PHOTO_CAP).map((r) => mediaPublicPath(r.media_id))
}
