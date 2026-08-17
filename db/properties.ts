import { sql } from "./client"
import type {
  GalleryImage,
  PropertyDetail,
  PropertyDetailResult,
  PropertyVideo,
} from "@/lib/property-types"

type PropertyMediaRow = {
  media_id: string
  media_type: "image" | "video"
  role: "hero" | "gallery" | "video" | "short"
  sort_order: number
  filename: string | null
  alt_text: string | null
  caption: string | null
  mux_playback_id: string | null
  aspect_ratio: string | null
  duration_seconds: string | null
}

type PropertyRow = {
  id: string
  name: string
  slug: string | null
  listing_identifier: string | null
  status: string
  property_type: string | null
  list_price: string | null
  featured: boolean

  city: string | null
  state_or_province: string | null
  neighborhood: string | null
  latitude: string | null
  longitude: string | null

  bedrooms: string | null
  bathrooms: string | null
  bathrooms_full: number | null
  bathrooms_half: number | null
  square_feet: number | null

  lot_size: string | null
  lot_size_units: string | null
  year_built: number | null
  stories: string | null
  parking_spaces: number | null

  has_ocean_view: boolean
  has_bay_view: boolean
  has_beach_view: boolean
  has_harbor_view: boolean
  has_island_view: boolean
  has_mountain_view: boolean
  has_sunrise_view: boolean
  has_sunset_view: boolean

  has_water_access: boolean
  has_beach_access: boolean

  has_pool: boolean
  has_generator: boolean
  has_solar: boolean
  is_furnished: boolean
  is_gated: boolean

  short_description: string | null
  editorial_description: string | null
  architecture_notes: string | null
  amenities_notes: string | null
  lifestyle_notes: string | null

  listing_agent_name: string | null
  listing_agent_email: string | null
  listing_agent_phone: string | null
  listing_office: string | null

  media: PropertyMediaRow[] | null
}

type PropertySummaryRow = {
  id: string
  name: string
  slug: string | null
  status: string
  property_type: string | null
  list_price: string | null
  featured: boolean

  location: string | null
  city: string | null
  neighborhood: string | null

  bedrooms: string | null
  bathrooms: string | null
  square_feet: number | null

  lot_size: string | null
  lot_size_units: string | null

  has_ocean_view: boolean
  has_bay_view: boolean
  has_beach_view: boolean
  has_harbor_view: boolean
  has_island_view: boolean
  has_mountain_view: boolean
  has_sunrise_view: boolean
  has_sunset_view: boolean
  has_water_access: boolean
  has_beach_access: boolean

  hero_media_id: string | null
  hero_alt_text: string | null
}

export type PropertySummary = {
  id: string
  name: string
  slug: string
  status: string
  propertyType: string | null
  listPrice: number | null
  featured: boolean

  location: string | null
  city: string | null
  neighborhood: string | null

  bedrooms: number | null
  bathrooms: number | null
  squareFeet: number | null

  lotSize: number | null
  lotSizeUnits: string | null

  views: string[]

  waterAccess: boolean
  beachAccess: boolean

  heroUrl: string | null
  heroAlt: string
}

function toNumber(value: string | null) {
  return value === null ? null : Number(value)
}

function statusLabel(status: string) {
  switch (status) {
    case "coming_soon":
      return "Coming Soon"
    case "under_contract":
      return "Pending"
    case "sold":
      return "Closed"
    case "off_market":
      return "Private"
    case "archived":
      return "Archived"
    case "prospect":
      return "Prospect"
    default:
      return "Active"
  }
}

function buildViewTypes(row: {
  has_ocean_view: boolean
  has_bay_view: boolean
  has_beach_view: boolean
  has_harbor_view: boolean
  has_island_view: boolean
  has_mountain_view: boolean
  has_sunrise_view: boolean
  has_sunset_view: boolean
}) {
  const views: string[] = []

  if (row.has_ocean_view) views.push("Ocean")
  if (row.has_bay_view) views.push("Bay")
  if (row.has_beach_view) views.push("Beach")
  if (row.has_harbor_view) views.push("Harbor")
  if (row.has_island_view) views.push("Island")
  if (row.has_mountain_view) views.push("Mountain")
  if (row.has_sunrise_view) views.push("Sunrise")
  if (row.has_sunset_view) views.push("Sunset")

  return views
}

function buildAmenities(row: PropertyRow) {
  const amenities: string[] = []

  if (row.has_pool) amenities.push("Pool")
  if (row.has_generator) amenities.push("Whole-Home Generator")
  if (row.has_solar) amenities.push("Solar Power")
  if (row.is_furnished) amenities.push("Furnished")
  if (row.is_gated) amenities.push("Gated")
  if (row.has_water_access) amenities.push("Water Access")
  if (row.has_beach_access) amenities.push("Beach Access")

  if (row.amenities_notes) {
    amenities.push(row.amenities_notes)
  }

  return amenities
}

function buildLifestyleTags(row: PropertyRow) {
  const tags: string[] = []

  if (row.has_ocean_view) tags.push("Ocean View")
  if (row.has_sunset_view) tags.push("Sunset View")
  if (row.has_sunrise_view) tags.push("Sunrise View")
  if (row.has_beach_access) tags.push("Beach Access")
  if (row.has_water_access) tags.push("Water Access")
  if (row.has_pool) tags.push("Pool")
  if (row.is_gated) tags.push("Private Estate")

  if (row.lifestyle_notes) {
    tags.push(row.lifestyle_notes)
  }

  return tags
}


/* ============================================================
   BUYERS / INVENTORY
   ============================================================ */

export async function getProperties(): Promise<PropertySummary[]> {
  const rows = await sql`
    select
      p.id,
      p.name,
      p.slug,
      p.status,
      p.property_type,
      p.list_price,
      p.featured,

      p.location,
      p.city,
      p.neighborhood,

      p.bedrooms,
      p.bathrooms,
      p.square_feet,

      p.lot_size,
      p.lot_size_units,

      p.has_ocean_view,
      p.has_bay_view,
      p.has_beach_view,
      p.has_harbor_view,
      p.has_island_view,
      p.has_mountain_view,
      p.has_sunrise_view,
      p.has_sunset_view,
      p.has_water_access,
      p.has_beach_access,

      hero.media_id as hero_media_id,
      hero.alt_text as hero_alt_text

    from property p

    left join lateral (
      select
        m.id as media_id,
        m.alt_text
      from property_media pm
      join media m
        on m.id = pm.media_id
      where pm.property_id = p.id
        and pm.role = 'hero'
        and m.media_type = 'image'
      order by
        pm.sort_order asc,
        pm.created_at asc
      limit 1
    ) hero on true

    where p.archived_at is null
      and p.slug is not null
      and p.status in (
        'active',
        'coming_soon',
        'under_contract'
      )

    order by
      p.featured desc,
      p.list_price desc nulls last,
      p.created_at desc
  `

  return (rows as PropertySummaryRow[]).map((row) => ({
    id: row.id,
    name: row.name,
    slug: row.slug!,
    status: statusLabel(row.status),
    propertyType: row.property_type,
    listPrice: toNumber(row.list_price),
    featured: row.featured,

    location: row.location,
    city: row.city,
    neighborhood: row.neighborhood,

    bedrooms: toNumber(row.bedrooms),
    bathrooms: toNumber(row.bathrooms),
    squareFeet: row.square_feet,

    lotSize: toNumber(row.lot_size),
    lotSizeUnits: row.lot_size_units,

    views: buildViewTypes(row),

    waterAccess: row.has_water_access,
    beachAccess: row.has_beach_access,

    heroUrl: row.hero_media_id
      ? `/api/media/${row.hero_media_id}`
      : null,

    heroAlt:
      row.hero_alt_text ??
      row.name,
  }))
}


/* ============================================================
   PROPERTY DETAIL
   ============================================================ */

export async function getPropertyBySlug(
  slug: string
): Promise<PropertyDetailResult | null> {
  const rows = await sql`
    select
      p.id,
      p.name,
      p.slug,
      p.listing_identifier,
      p.status,
      p.property_type,
      p.list_price,
      p.featured,

      p.city,
      p.state_or_province,
      p.neighborhood,
      p.latitude,
      p.longitude,

      p.bedrooms,
      p.bathrooms,
      p.bathrooms_full,
      p.bathrooms_half,
      p.square_feet,

      p.lot_size,
      p.lot_size_units,
      p.year_built,
      p.stories,
      p.parking_spaces,

      p.has_ocean_view,
      p.has_bay_view,
      p.has_beach_view,
      p.has_harbor_view,
      p.has_island_view,
      p.has_mountain_view,
      p.has_sunrise_view,
      p.has_sunset_view,

      p.has_water_access,
      p.has_beach_access,

      p.has_pool,
      p.has_generator,
      p.has_solar,
      p.is_furnished,
      p.is_gated,

      p.short_description,
      p.editorial_description,
      p.architecture_notes,
      p.amenities_notes,
      p.lifestyle_notes,

      p.listing_agent_name,
      p.listing_agent_email,
      p.listing_agent_phone,
      p.listing_office,

      coalesce(
        (
          select jsonb_agg(
            jsonb_build_object(
              'media_id', m.id,
              'media_type', m.media_type,
              'role', pm.role,
              'sort_order', pm.sort_order,
              'filename', m.filename,
              'alt_text', m.alt_text,
              'caption', m.caption,
              'mux_playback_id', m.mux_playback_id,
              'aspect_ratio', m.aspect_ratio,
              'duration_seconds', m.duration_seconds::text
            )
            order by
              case pm.role
                when 'hero' then 0
                when 'gallery' then 1
                when 'video' then 2
                when 'short' then 3
                else 4
              end,
              pm.sort_order asc,
              pm.created_at asc
          )
          from property_media pm

          join media m
            on m.id = pm.media_id

          where pm.property_id = p.id
        ),
        '[]'::jsonb
      ) as media

    from property p

    where p.slug = ${slug}
      and p.archived_at is null

    limit 1
  `

  if (rows.length === 0) {
    return null
  }

  const row = rows[0] as PropertyRow
  const media = row.media ?? []

  const hero = media.find(
    (item) =>
      item.media_type === "image" &&
      item.role === "hero"
  )

  const galleryImages: GalleryImage[] = media
    .filter(
      (item) =>
        item.media_type === "image" &&
        (item.role === "hero" || item.role === "gallery")
    )
    .map((item) => ({
      url: `/api/media/${item.media_id}`,
      alt: item.alt_text ?? row.name,
      caption: item.caption ?? null,
    }))

  const videos: PropertyVideo[] = media
    .filter(
      (item) =>
        item.media_type === "video" &&
        (item.role === "video" || item.role === "short") &&
        item.mux_playback_id
    )
    .map((item) => ({
      id: item.media_id,
      playbackId: item.mux_playback_id!,
      role: item.role as "video" | "short",
      title: item.caption ?? row.name,
      caption: item.caption ?? null,
      aspectRatio: item.aspect_ratio ?? null,
      durationSeconds: toNumber(item.duration_seconds),
    }))

  const property: PropertyDetail = {
    _id: row.id,
    title: row.name,
    listingId: row.listing_identifier,
    standardStatus: statusLabel(row.status),
    propertyType: row.property_type,
    listPrice: toNumber(row.list_price),

    city: row.city,
    stateOrProvince: row.state_or_province,
    neighborhood: row.neighborhood,
    latitude: toNumber(row.latitude),
    longitude: toNumber(row.longitude),

    bedroomsTotal: toNumber(row.bedrooms),
    bathroomsFull: row.bathrooms_full,
    bathroomsHalf: row.bathrooms_half,
    bathroomsTotal: toNumber(row.bathrooms),
    livingArea: row.square_feet,

    lotSizeArea: toNumber(row.lot_size),
    lotSizeUnits: row.lot_size_units,
    yearBuilt: row.year_built,
    stories: toNumber(row.stories),
    parkingSpaces: row.parking_spaces,

    viewType: buildViewTypes(row),
    waterAccess: row.has_water_access,
    beachAccess: row.has_beach_access,

    amenities: buildAmenities(row),

    shortDescription: row.short_description,
    editorialDescription: row.editorial_description,

    architecture: row.architecture_notes,
    lifestyleTags: buildLifestyleTags(row),

    listingAgentName: row.listing_agent_name,
    listingAgentEmail: row.listing_agent_email,
    listingAgentPhone: row.listing_agent_phone,
    listingOffice: row.listing_office,
  }

  return {
    property,
    heroUrl: hero
      ? `/api/media/${hero.media_id}`
      : null,
    galleryImages,
    videos,
  }
}
