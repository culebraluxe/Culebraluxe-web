import { sql } from './client'
import { getPropertyMediaCoverage } from './property-media-coverage'

// Read-only media/documents audit projection (OPS-04). Answers which
// properties have hero/gallery/video/document media and which media are
// orphaned (not linked to any property or guide item). No uploads, deletes,
// reordering, or role changes.

export type MediaTypeSummary = {
  mediaType: string
  total: number
  linked: number
  unlinked: number
  missingAlt: number
}

export type PropertyMediaRow = {
  propertyId: string
  propertyName: string
  hasHero: boolean
  imageCount: number
  videoCount: number
  documentCount: number
  totalCount: number
}

export type MediaAdminSnapshot = {
  totalMedia: number
  totalUnlinked: number
  byType: MediaTypeSummary[]
  propertyRows: PropertyMediaRow[]
}

type MediaTypeRow = {
  media_type: string
  total: number
  linked: number
  unlinked: number
  missing_alt: number
}

export async function getMediaAdmin(): Promise<MediaAdminSnapshot> {
  const [typeRows, coverageRows] = await Promise.all([
    sql`
      select
        m.media_type,
        count(*)::int as total,
        count(*) filter (
          where exists (
            select 1 from property_media pm where pm.media_id = m.id
          )
          or exists (
            select 1 from guide_item_media gim where gim.media_id = m.id
          )
        ) as linked,
        count(*) filter (
          where not exists (
            select 1 from property_media pm where pm.media_id = m.id
          )
          and not exists (
            select 1 from guide_item_media gim where gim.media_id = m.id
          )
        ) as unlinked,
        count(*) filter (
          where m.alt_text is null or m.alt_text = ''
        ) as missing_alt
      from media m
      group by m.media_type
      order by m.media_type
    `,
    getPropertyMediaCoverage(),
  ])

  const byType = (typeRows as MediaTypeRow[]).map((row) => ({
    mediaType: row.media_type,
    total: row.total,
    linked: row.linked,
    unlinked: row.unlinked,
    missingAlt: row.missing_alt,
  }))

  const totalMedia = byType.reduce((sum, item) => sum + item.total, 0)
  const totalUnlinked = byType.reduce((sum, item) => sum + item.unlinked, 0)

  const propertyRows = coverageRows
    .filter(
      (row) =>
        !row.archived &&
        row.imageCount + row.videoCount + row.documentCount > 0,
    )
    .map((row) => ({
      propertyId: row.propertyId,
      propertyName: row.propertyName,
      hasHero: row.heroMediaId !== null,
      imageCount: row.imageCount,
      videoCount: row.videoCount,
      documentCount: row.documentCount,
      totalCount: row.imageCount + row.videoCount + row.documentCount,
    }))

  return {
    totalMedia,
    totalUnlinked,
    byType,
    propertyRows,
  }
}
