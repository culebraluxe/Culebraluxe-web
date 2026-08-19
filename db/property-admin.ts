import { sql } from './client'

// Read-only property administration projection (OPS-03). One row per property
// (including archived) with brokerage-inventory admin fields. No edits, no
// uploads, no status changes.

export type PropertyAdminRow = {
  id: string
  name: string
  slug: string | null
  status: string
  featured: boolean
  listPrice: number | null
  location: string | null
  bedrooms: number | null
  bathrooms: number | null
  squareFeet: number | null
  propertyType: string | null
  sellerName: string | null
  heroMediaId: string | null
  imageCount: number
  videoCount: number
  documentCount: number
  archived: boolean
}

type PropertyAdminRowRaw = {
  id: string
  name: string
  slug: string | null
  status: string
  featured: boolean
  list_price: string | null
  location: string | null
  bedrooms: string | null
  bathrooms: string | null
  square_feet: number | null
  property_type: string | null
  seller_name: string | null
  hero_media_id: string | null
  image_count: number
  video_count: number
  document_count: number
  archived_at: string | null
}

function toNumber(value: string | null) {
  return value === null ? null : Number(value)
}

export async function getPropertyAdmin(): Promise<PropertyAdminRow[]> {
  const rows = await sql`
    select
      p.id,
      p.name,
      p.slug,
      p.status,
      p.featured,
      p.list_price,
      p.location,
      p.bedrooms,
      p.bathrooms,
      p.square_feet,
      p.property_type,
      p.archived_at,
      seller.display_name as seller_name,
      hero.media_id as hero_media_id,
      counts.image_count,
      counts.video_count,
      counts.document_count
    from property p
    left join person seller
      on seller.id = p.seller_person_id
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

  return (rows as PropertyAdminRowRaw[]).map((row) => ({
    id: row.id,
    name: row.name,
    slug: row.slug ?? null,
    status: row.status,
    featured: row.featured,
    listPrice: toNumber(row.list_price),
    location: row.location ?? null,
    bedrooms: toNumber(row.bedrooms),
    bathrooms: toNumber(row.bathrooms),
    squareFeet: row.square_feet,
    propertyType: row.property_type ?? null,
    sellerName: row.seller_name ?? null,
    heroMediaId: row.hero_media_id ?? null,
    imageCount: row.image_count,
    videoCount: row.video_count,
    documentCount: row.document_count,
    archived: row.archived_at !== null,
  }))
}
