import { sql } from "./client"

export type GuideItem = {
  id: string
  slug: string
  section: string
  name: string
  eyebrow: string | null
  subtitle: string | null
  area: string | null
  description: string
  note: string | null
  address: string | null
  phone: string | null
  websiteUrl: string | null
  latitude: number | null
  longitude: number | null
  sortOrder: number
  imageUrl: string | null
  imageAlt: string | null
}

type GuideItemRow = {
  id: string
  slug: string
  section: string
  name: string
  eyebrow: string | null
  subtitle: string | null
  area: string | null
  description: string
  note: string | null
  address: string | null
  phone: string | null
  website_url: string | null
  latitude: string | null
  longitude: string | null
  sort_order: number
  card_media_id: string | null
  card_alt_text: string | null
}

const SECTION_ORDER = [
  "beaches",
  "water",
  "wildlife-land",
  "coffee-casual",
  "dining",
  "getting-here",
  "getting-around",
  "essentials",
  "island-story",
] as const

function toNumber(value: string | null) {
  if (value === null) return null
  const parsed = Number(value)
  return Number.isFinite(parsed) ? parsed : null
}

export async function getGuideItems(): Promise<GuideItem[]> {
  const rows = await sql`
    select
      gi.id,
      gi.slug,
      gi.section,
      gi.name,
      gi.eyebrow,
      gi.subtitle,
      gi.area,
      gi.description,
      gi.note,
      gi.address,
      gi.phone,
      gi.website_url,
      gi.latitude,
      gi.longitude,
      gi.sort_order,
      card.media_id as card_media_id,
      card.alt_text as card_alt_text
    from guide_item gi
    left join lateral (
      select
        m.id as media_id,
        m.alt_text
      from guide_item_media gim
      join media m
        on m.id = gim.media_id
      where gim.guide_item_id = gi.id
        and gim.role = 'card'
        and m.media_type = 'image'
      order by
        gim.sort_order asc,
        gim.created_at asc
      limit 1
    ) card on true
    where gi.is_active = true
    order by
      case gi.section
        when 'beaches' then 1
        when 'water' then 2
        when 'wildlife-land' then 3
        when 'coffee-casual' then 4
        when 'dining' then 5
        when 'getting-here' then 6
        when 'getting-around' then 7
        when 'essentials' then 8
        when 'island-story' then 9
        else 99
      end,
      gi.sort_order asc,
      gi.name asc
  `

  return (rows as GuideItemRow[]).map((row) => ({
    id: row.id,
    slug: row.slug,
    section: row.section,
    name: row.name,
    eyebrow: row.eyebrow,
    subtitle: row.subtitle,
    area: row.area,
    description: row.description,
    note: row.note,
    address: row.address,
    phone: row.phone,
    websiteUrl: row.website_url,
    latitude: toNumber(row.latitude),
    longitude: toNumber(row.longitude),
    sortOrder: row.sort_order,
    imageUrl: row.card_media_id ? `/api/media/${row.card_media_id}` : null,
    imageAlt: row.card_alt_text ?? row.name,
  }))
}

export { SECTION_ORDER }