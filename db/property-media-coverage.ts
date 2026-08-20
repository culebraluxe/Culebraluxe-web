import { sql } from './client'

// Read-only shared projection (ENG-02B) for per-property media coverage
// (hero/gallery/video/document) used by Property Admin and Media Audit.
// Does not touch upload, delete, reorder, or role-change paths.

export type PropertyMediaCoverageRow = {
  propertyId: string
  propertyName: string
  archived: boolean
  heroMediaId: string | null
  imageCount: number
  videoCount: number
  documentCount: number
}

type PropertyMediaCoverageRaw = {
  property_id: string
  property_name: string
  archived_at: string | null
  hero_media_id: string | null
  image_count: number
  video_count: number
  document_count: number
}

export async function getPropertyMediaCoverage(): Promise<PropertyMediaCoverageRow[]> {
  const rows = await sql`
    select
      p.id as property_id,
      p.name as property_name,
      p.archived_at,
      hero.media_id as hero_media_id,
      counts.image_count,
      counts.video_count,
      counts.document_count
    from property p
    left join lateral (
      select pm.media_id
      from property_media pm
      join media m
        on m.id = pm.media_id
      where pm.property_id = p.id
        and pm.role = 'hero'
        and m.media_type = 'image'
      order by pm.sort_order asc, pm.created_at asc
      limit 1
    ) hero on true
    left join lateral (
      select
        count(*) filter (where m.media_type = 'image') as image_count,
        count(*) filter (where m.media_type = 'video') as video_count,
        count(*) filter (where m.media_type = 'document') as document_count
      from property_media pm
      join media m
        on m.id = pm.media_id
      where pm.property_id = p.id
    ) counts on true
    order by
      case when p.archived_at is null then 0 else 1 end,
      p.name asc
  `

  return (rows as PropertyMediaCoverageRaw[]).map((row) => ({
    propertyId: row.property_id,
    propertyName: row.property_name,
    archived: row.archived_at !== null,
    heroMediaId: row.hero_media_id ?? null,
    imageCount: row.image_count,
    videoCount: row.video_count,
    documentCount: row.document_count,
  }))
}
