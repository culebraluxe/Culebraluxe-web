import { sql } from './client'
import { getPropertyMediaCoverage } from './property-media-coverage'

// Read-only property administration projection (OPS-03). One row per property
// (including archived) with brokerage-inventory admin fields. No edits, no
// uploads, no status changes.

export type PropertyDataQuality = {
  missingSlug: boolean
  missingPrice: boolean
  missingLocation: boolean
  missingDescription: boolean
  missingBedsBaths: boolean
  noMedia: boolean
  missingHero: boolean
  malformedLotUnits: boolean
}

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
  dataQuality: PropertyDataQuality
  issueCount: number
}

type PropertyAdminBaseRaw = {
  id: string
  name: string
  slug: string | null
  status: string
  featured: boolean
  list_price: string | null
  location: string | null
  city: string | null
  neighborhood: string | null
  bedrooms: string | null
  bathrooms: string | null
  square_feet: number | null
  property_type: string | null
  lot_size: string | null
  lot_size_units: string | null
  short_description: string | null
  editorial_description: string | null
  seller_name: string | null
  archived_at: string | null
}

function toNumber(value: string | null) {
  return value === null ? null : Number(value)
}

export async function getPropertyAdmin(): Promise<PropertyAdminRow[]> {
  const [baseRows, coverageRows] = await Promise.all([
    sql`
      select
        p.id,
        p.name,
        p.slug,
        p.status,
        p.featured,
        p.list_price,
        p.location,
        p.city,
        p.neighborhood,
        p.bedrooms,
        p.bathrooms,
        p.square_feet,
        p.property_type,
        p.lot_size,
        p.lot_size_units,
        p.short_description,
        p.editorial_description,
        p.archived_at,
        seller.display_name as seller_name
      from property p
      left join person seller
        on seller.id = p.seller_person_id
      order by
        case when p.archived_at is null then 0 else 1 end,
        p.name asc
    `,
    getPropertyMediaCoverage(),
  ])

  const coverageByProperty = new Map(
    coverageRows.map((row) => [row.propertyId, row]),
  )

  return (baseRows as PropertyAdminBaseRaw[]).map((row) => {
    const coverage = coverageByProperty.get(row.id)
    const totalMedia =
      (coverage?.imageCount ?? 0) +
      (coverage?.videoCount ?? 0) +
      (coverage?.documentCount ?? 0)
    const isHome =
      row.property_type === null ||
      row.property_type.toLowerCase() !== 'land'

    const dataQuality: PropertyDataQuality = {
      missingSlug: row.slug === null,
      missingPrice: row.list_price === null,
      missingLocation:
        row.location === null &&
        row.city === null &&
        row.neighborhood === null,
      missingDescription:
        row.short_description === null ||
        row.editorial_description === null,
      missingBedsBaths:
        isHome && (row.bedrooms === null || row.bathrooms === null),
      noMedia: totalMedia === 0,
      missingHero: totalMedia > 0 && coverage?.heroMediaId === null,
      malformedLotUnits:
        row.lot_size !== null && row.lot_size_units === null,
    }

    const issueCount = Object.values(dataQuality).filter(Boolean).length

    return {
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
      heroMediaId: coverage?.heroMediaId ?? null,
      imageCount: coverage?.imageCount ?? 0,
      videoCount: coverage?.videoCount ?? 0,
      documentCount: coverage?.documentCount ?? 0,
      archived: row.archived_at !== null,
      dataQuality,
      issueCount,
    }
  })
}
