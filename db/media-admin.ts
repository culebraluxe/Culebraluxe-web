import { sql } from './client'

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

type PropertyMediaRowRaw = {
  property_id: string
  property_name: string
  has_hero: boolean
  image_count: number
  video_count: number
  document_count: number
  total_count: number
}

export async function getMediaAdmin(): Promise<MediaAdminSnapshot> {
  const [typeRows, propertyRows] = await Promise.all([
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
    sql`
      select
        p.id as property_id,
        p.name as property_name,
        bool_or(
          m.media_type = 'image' and pm.role = 'hero'
        ) as has_hero,
        count(*) filter (where m.media_type = 'image') as image_count,
        count(*) filter (where m.media_type = 'video') as video_count,
        count(*) filter (where m.media_type = 'document') as document_count,
        count(*)::int as total_count
      from property_media pm
      join media m
        on m.id = pm.media_id
      join property p
        on p.id = pm.property_id
      where p.archived_at is null
      group by p.id, p.name
      order by p.name asc
    `,
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

  return {
    totalMedia,
    totalUnlinked,
    byType,
    propertyRows: (propertyRows as PropertyMediaRowRaw[]).map((row) => ({
      propertyId: row.property_id,
      propertyName: row.property_name,
      hasHero: row.has_hero,
      imageCount: row.image_count,
      videoCount: row.video_count,
      documentCount: row.document_count,
      totalCount: row.total_count,
    })),
  }
}
