import { sql } from "./client"
import type {
  GalleryImage,
  PropertyDetail,
  PropertyDetailResult,
  PropertyDocument,
  PropertyVideo,
} from "@/lib/property-types"

type PropertyMediaRow = {
  media_id: string
  media_type: "image" | "video" | "document"
  role: "hero" | "gallery" | "video" | "short" | "document"
  sort_order: number
  filename: string | null
  mime_type: string | null
  file_size: string | null
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

export type PropertyIntro = {
  id: string
  name: string
  location: string | null
}

export async function getPropertyIntroById(
  propertyId: string,
): Promise<PropertyIntro | null> {
  const rows = await sql`
    select id, name, location
    from property
    where id = ${propertyId}
      and archived_at is null
    limit 1
  `
  const row = rows[0] as PropertyIntro | undefined
  return row
    ? { id: row.id, name: row.name, location: row.location ?? null }
    : null
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

  return (rows as PropertySummaryRow[]).map(mapSummary)
}

function mapSummary(row: PropertySummaryRow): PropertySummary {
  return {
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
  }
}

export type PropertyFilterInput = {
  category?: 'all' | 'homes' | 'land'
  q?: string
  maxPrice?: number | null
  beds?: number | null
  view?: string
  sort?: 'featured' | 'price-high' | 'price-low' | 'name'
}

// Server-side buyers filter contract (PX-24B). Filters are applied in SQL
// over canonical property fields only; free-text search and sorting remain
// deterministic client-side conveniences over the already-filtered result so
// structured filters never depend on full-inventory client logic. Returns the
// matching inventory plus the distinct view vocabulary across all eligible
// inventory (so the view dropdown stays stable while filtering).
export async function getFilteredProperties(
  filters: PropertyFilterInput,
): Promise<{ properties: PropertySummary[]; viewOptions: string[] }> {
  const category = filters.category ?? 'all'
  const q = (filters.q ?? '').trim().toLowerCase()
  const maxPrice = filters.maxPrice ?? null
  const beds = filters.beds ?? null
  const view = filters.view?.trim() ?? ''
  const viewValue = view.toLowerCase()
  const hasView = [
    'ocean',
    'bay',
    'beach',
    'harbor',
    'island',
    'mountain',
    'sunrise',
    'sunset',
  ].includes(viewValue)

  const propertyRows = await sql`
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
      and p.status in ('active', 'coming_soon', 'under_contract')

      and (${category !== 'land'} or lower(coalesce(p.property_type, '')) = 'land')
      and (${category !== 'homes'} or lower(coalesce(p.property_type, '')) <> 'land')

      and (${maxPrice === null} or p.list_price <= ${maxPrice})

      and (
        ${beds === null}
        or (lower(coalesce(p.property_type, '')) <> 'land' and p.bedrooms >= ${beds})
      )

      and (
        ${!hasView}
        or case
          when ${viewValue} = 'ocean' then p.has_ocean_view
          when ${viewValue} = 'bay' then p.has_bay_view
          when ${viewValue} = 'beach' then p.has_beach_view
          when ${viewValue} = 'harbor' then p.has_harbor_view
          when ${viewValue} = 'island' then p.has_island_view
          when ${viewValue} = 'mountain' then p.has_mountain_view
          when ${viewValue} = 'sunrise' then p.has_sunrise_view
          when ${viewValue} = 'sunset' then p.has_sunset_view
          else false
        end
      )

      and (
        ${q === ''}
        or lower(
          coalesce(p.name, '') || ' ' ||
          coalesce(p.location, '') || ' ' ||
          coalesce(p.city, '') || ' ' ||
          coalesce(p.neighborhood, '') || ' ' ||
          coalesce(p.property_type, '') ||
          case when p.has_ocean_view then ' ocean' else '' end ||
          case when p.has_bay_view then ' bay' else '' end ||
          case when p.has_beach_view then ' beach' else '' end ||
          case when p.has_harbor_view then ' harbor' else '' end ||
          case when p.has_island_view then ' island' else '' end ||
          case when p.has_mountain_view then ' mountain' else '' end ||
          case when p.has_sunrise_view then ' sunrise' else '' end ||
          case when p.has_sunset_view then ' sunset' else '' end
        ) like '%' || ${q} || '%'
      )

    order by
      p.featured desc,
      p.list_price desc nulls last,
      p.created_at desc
  `

  const viewRows = await sql`
    select
      has_ocean_view,
      has_bay_view,
      has_beach_view,
      has_harbor_view,
      has_island_view,
      has_mountain_view,
      has_sunrise_view,
      has_sunset_view
    from property
    where archived_at is null
      and slug is not null
      and status in ('active', 'coming_soon', 'under_contract')
  `

  const viewSet = new Set<string>()
  for (const row of viewRows as Array<Record<string, boolean>>) {
    for (const viewName of buildViewTypes(row as Parameters<typeof buildViewTypes>[0])) {
      viewSet.add(viewName)
    }
  }

  return {
    properties: (propertyRows as PropertySummaryRow[]).map(mapSummary),
    viewOptions: Array.from(viewSet).sort(),
  }
}

// Deterministic, explainable "similar" rule using only canonical property
// fields: same property type first, then same neighborhood/city, then price
// proximity, with the standard inventory ordering as a stable tiebreak. No
// similarity score is computed or exposed; the current property is excluded
// and only active/public inventory is eligible.
export async function getSimilarProperties(
  propertyId: string,
  current: {
    propertyType: string | null
    city: string | null
    neighborhood: string | null
    listPrice: number | null
  },
  limit = 3,
): Promise<PropertySummary[]> {
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

    where p.id <> ${propertyId}
      and p.archived_at is null
      and p.slug is not null
      and p.status in ('active', 'coming_soon', 'under_contract')

    order by
      case when p.property_type = ${current.propertyType} then 0 else 1 end,
      case
        when ${current.neighborhood} is not null
          and p.neighborhood = ${current.neighborhood} then 0
        when ${current.neighborhood} is null
          and ${current.city} is not null
          and p.city = ${current.city} then 0
        else 1
      end,
      case
        when ${current.listPrice} is not null
          then abs(coalesce(p.list_price, 0) - ${current.listPrice})
        else 0
      end asc,
      p.featured desc,
      p.list_price desc nulls last,
      p.created_at desc

    limit ${limit}
  `

  return (rows as PropertySummaryRow[]).map(mapSummary)
}

// Lightweight public-slug set used by browser-local "Recently Viewed" so
// stored entries can be checked against currently live listings and stale
// or missing slugs can be dropped safely.
export async function getPublicPropertySlugs(): Promise<string[]> {
  const rows = await sql`
    select slug
    from property
    where archived_at is null
      and slug is not null
      and status in ('active', 'coming_soon', 'under_contract')
    order by created_at asc
  `
  return (rows as { slug: string }[]).map((row) => row.slug)
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
              'mime_type', m.mime_type,
              'file_size', m.file_size::text,
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
                when 'document' then 4
                else 5
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

  const documents: PropertyDocument[] = media
    .filter(
      (item) =>
        item.media_type === "document" &&
        item.role === "document" &&
        item.filename &&
        item.mime_type
    )
    .map((item) => ({
      id: item.media_id,
      title: item.caption ?? item.alt_text ?? item.filename!,
      filename: item.filename!,
      mimeType: item.mime_type!,
      fileSize: toNumber(item.file_size),
      sortOrder: item.sort_order,
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
    documents,
  }
}
