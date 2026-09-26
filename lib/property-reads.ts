import 'server-only'

import { randomUUID } from 'node:crypto'

import type { PropertyDetailResult } from '@/lib/property-types'
import {
  rustApiPublicListings,
  rustApiPublicProperty,
  rustApiPublicSimilar,
  rustApiPublicSlugs,
  rustApiRead,
} from '@/lib/rust-api/client'
import type {
  RustPublicListing,
  RustPublicProperty,
} from '@/lib/rust-api/client'

export type DbFailureKind =
  | 'DATABASE_UNAVAILABLE'
  | 'SCHEMA_MISMATCH'
  | 'CONSTRAINT'
  | 'TIMEOUT'
  | 'UNKNOWN'

export type DbFailure = {
  kind: DbFailureKind
  operation: string
  incidentId: string
  code?: string
  detail?: string
  retryable?: boolean
}

export type Result<T> = { ok: true; data: T } | { ok: false; error: DbFailure }

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

export type PropertyFilterInput = {
  category?: 'all' | 'homes' | 'land'
  q?: string
  maxPrice?: number | null
  beds?: number | null
  view?: string
  sort?: 'featured' | 'price-high' | 'price-low' | 'name'
}

type RustPropertyAdminPage = {
  rows: Array<{
    id: string
    status: string
    archived: boolean
  }>
  total: number
  page: number
  pageSize: number
}

type RustPropertyAdminRecord = {
  id: string
  name: string
  slug: string | null
  status: string
  featured: boolean
  propertyType: string | null
  hasOceanView: boolean
  hasBayView: boolean
  hasBeachView: boolean
  hasHarborView: boolean
  hasIslandView: boolean
  hasMountainView: boolean
  hasSunriseView: boolean
  hasSunsetView: boolean
  hasWaterAccess: boolean
  hasBeachAccess: boolean
  listPrice: string | null
  location: string | null
  city: string | null
  neighborhood: string | null
  bedrooms: string | null
  bathrooms: string | null
  squareFeet: string | null
  lotSize: string | null
  lotSizeAcres: string | null
  lotSizeUnits: string | null
}

function failure(operation: string, error: unknown): Result<never> {
  const value = error as {
    code?: string
    message?: string
    retryable?: boolean
    correlationId?: string
  }
  return {
    ok: false,
    error: {
      kind: 'UNKNOWN',
      operation,
      incidentId: value.correlationId ?? randomUUID(),
      code: value.code,
      detail: value.message ?? 'Rust API read failed.',
      retryable: value.retryable,
    },
  }
}

function statusLabel(status: string) {
  switch (status) {
    case 'coming_soon':
      return 'Coming Soon'
    case 'under_contract':
      return 'Pending'
    case 'sold':
      return 'Closed'
    case 'off_market':
      return 'Private'
    case 'archived':
      return 'Archived'
    case 'prospect':
      return 'Prospect'
    default:
      return 'Active'
  }
}

function locationOf(value: {
  neighborhood?: string | null
  city?: string | null
  location?: string | null
}) {
  return (
    value.location ??
    ([value.neighborhood, value.city].filter(Boolean).join(', ') || null)
  )
}

function toNumber(value: string | number | null | undefined): number | null {
  if (value === null || value === undefined || value === '') return null
  const number = Number(value)
  return Number.isFinite(number) ? number : null
}

function summaryFromPublic(row: RustPublicListing): PropertySummary {
  return {
    id: row.id,
    name: row.name,
    slug: row.key,
    status: statusLabel(row.status),
    propertyType: row.propertyType,
    listPrice: row.listPrice,
    featured: row.featured,
    location: locationOf(row),
    city: row.city,
    neighborhood: row.neighborhood,
    bedrooms: row.bedrooms,
    bathrooms: row.bathrooms,
    squareFeet: row.squareFeet,
    lotSize: row.lotSize,
    lotSizeUnits: row.lotSizeUnits,
    views: row.views,
    waterAccess: row.waterAccess,
    beachAccess: row.beachAccess,
    heroUrl: row.heroMediaId ? `/api/media/${row.heroMediaId}` : null,
    heroAlt: row.heroAlt ?? row.name,
  }
}

function summaryFromAdmin(row: RustPropertyAdminRecord): PropertySummary {
  const views = [
    [row.hasOceanView, 'Ocean'],
    [row.hasBayView, 'Bay'],
    [row.hasBeachView, 'Beach'],
    [row.hasHarborView, 'Harbor'],
    [row.hasIslandView, 'Island'],
    [row.hasMountainView, 'Mountain'],
    [row.hasSunriseView, 'Sunrise'],
    [row.hasSunsetView, 'Sunset'],
  ]
    .filter(([enabled]) => enabled)
    .map(([, label]) => label as string)

  return {
    id: row.id,
    name: row.name,
    slug: row.slug ?? row.id,
    status: statusLabel(row.status),
    propertyType: row.propertyType,
    listPrice: toNumber(row.listPrice),
    featured: row.featured,
    location: locationOf(row),
    city: row.city,
    neighborhood: row.neighborhood,
    bedrooms: toNumber(row.bedrooms),
    bathrooms: toNumber(row.bathrooms),
    squareFeet: toNumber(row.squareFeet),
    lotSize: toNumber(row.lotSizeAcres ?? row.lotSize),
    lotSizeUnits: row.lotSizeAcres ? 'Acres' : row.lotSizeUnits,
    views,
    waterAccess: row.hasWaterAccess,
    beachAccess: row.hasBeachAccess,
    heroUrl: null,
    heroAlt: row.name,
  }
}

async function internalWorkingProperties(): Promise<PropertySummary[]> {
  const rows: RustPropertyAdminPage['rows'] = []
  const pageSize = 100
  for (let page = 1; ; page += 1) {
    const response = await rustApiRead<RustPropertyAdminPage>(
      `/v1/properties/admin?search=&page=${page}&page_size=${pageSize}`,
    )
    rows.push(...response.value.rows)
    if (rows.length >= response.value.total || response.value.rows.length === 0) break
  }

  const working = rows.filter(
    (row) =>
      !row.archived &&
      ['active', 'coming_soon', 'under_contract'].includes(row.status),
  )

  const details = await Promise.all(
    working.map(async (row) =>
      (
        await rustApiRead<RustPropertyAdminRecord>(
          `/v1/properties/${encodeURIComponent(row.id)}/admin`,
        )
      ).value,
    ),
  )
  return details.map(summaryFromAdmin)
}

export async function getProperties(opts: { publicOnly?: boolean } = {}): Promise<Result<PropertySummary[]>> {
  const operation = 'property.list'
  try {
    const rows = opts.publicOnly === true
      ? (await rustApiPublicListings()).map(summaryFromPublic)
      : await internalWorkingProperties()
    return { ok: true, data: rows }
  } catch (error) {
    return failure(operation, error)
  }
}

export async function getFilteredProperties(
  filters: PropertyFilterInput,
): Promise<Result<{ properties: PropertySummary[]; viewOptions: string[] }>> {
  const operation = 'property.search'
  try {
    const all = (await rustApiPublicListings()).map(summaryFromPublic)
    const category = filters.category ?? 'all'
    const query = (filters.q ?? '').trim().toLowerCase()
    const maxPrice = filters.maxPrice ?? null
    const beds = filters.beds ?? null
    const view = filters.view?.trim().toLowerCase() ?? ''

    const properties = all.filter((property) => {
      if (category === 'land' && property.propertyType?.toLowerCase() !== 'land') return false
      if (category === 'homes' && property.propertyType?.toLowerCase() === 'land') return false
      if (maxPrice !== null && (property.listPrice === null || property.listPrice > maxPrice)) return false
      if (
        beds !== null &&
        property.propertyType?.toLowerCase() !== 'land' &&
        (property.bedrooms === null || property.bedrooms < beds)
      ) return false
      if (view && !property.views.some((value) => value.toLowerCase() === view)) return false
      if (query) {
        const haystack = [
          property.name,
          property.location,
          property.city,
          property.neighborhood,
          property.propertyType,
          ...property.views,
        ]
          .filter(Boolean)
          .join(' ')
          .toLowerCase()
        if (!haystack.includes(query)) return false
      }
      return true
    })

    switch (filters.sort) {
      case 'price-high':
        properties.sort((a, b) => (b.listPrice ?? -Infinity) - (a.listPrice ?? -Infinity))
        break
      case 'price-low':
        properties.sort((a, b) => (a.listPrice ?? Infinity) - (b.listPrice ?? Infinity))
        break
      case 'name':
        properties.sort((a, b) => a.name.localeCompare(b.name))
        break
      default:
        properties.sort(
          (a, b) =>
            Number(b.featured) - Number(a.featured) ||
            (b.listPrice ?? -Infinity) - (a.listPrice ?? -Infinity) ||
            a.name.localeCompare(b.name),
        )
    }

    const viewOptions = Array.from(new Set(all.flatMap((property) => property.views))).sort()
    return { ok: true, data: { properties, viewOptions } }
  } catch (error) {
    return failure(operation, error)
  }
}

export async function getSimilarProperties(
  propertyId: string,
  _current: {
    propertyType: string | null
    city: string | null
    neighborhood: string | null
    listPrice: number | null
  },
  limit = 3,
): Promise<Result<PropertySummary[]>> {
  const operation = 'property.similar'
  try {
    return {
      ok: true,
      data: (await rustApiPublicSimilar(propertyId, limit)).map(summaryFromPublic),
    }
  } catch (error) {
    return failure(operation, error)
  }
}

export async function getPropertyIntroById(propertyId: string): Promise<Result<PropertyIntro | null>> {
  const operation = 'property.intro'
  try {
    const property = await rustApiPublicProperty(propertyId)
    return {
      ok: true,
      data: property
        ? { id: property.id, name: property.name, location: locationOf(property) }
        : null,
    }
  } catch (error) {
    return failure(operation, error)
  }
}

export async function getPublicPropertySlugs(): Promise<Result<string[]>> {
  const operation = 'property.publicSlugs'
  try {
    return { ok: true, data: await rustApiPublicSlugs() }
  } catch (error) {
    return failure(operation, error)
  }
}

function propertyDetailFromRust(row: RustPublicProperty): PropertyDetailResult {
  const galleryImages = row.media
    .filter((item) => item.mediaType === 'image' && (item.role === 'hero' || item.role === 'gallery'))
    .map((item) => ({
      url: `/api/media/${item.id}`,
      alt: item.altText ?? row.name,
      caption: item.caption ?? null,
    }))

  const videos = row.media
    .filter(
      (item) =>
        item.mediaType === 'video' &&
        (item.role === 'video' || item.role === 'short') &&
        item.muxPlaybackId,
    )
    .map((item) => ({
      id: item.id,
      playbackId: item.muxPlaybackId!,
      role: item.role as 'video' | 'short',
      title: item.caption ?? row.name,
      caption: item.caption ?? null,
      aspectRatio: item.aspectRatio ?? null,
      durationSeconds: item.durationSeconds ?? null,
    }))

  const documents = row.media
    .filter(
      (item) =>
        item.mediaType === 'document' &&
        item.role === 'document' &&
        item.filename &&
        item.mimeType,
    )
    .map((item) => ({
      id: item.id,
      title: item.caption ?? item.altText ?? item.filename!,
      filename: item.filename!,
      mimeType: item.mimeType!,
      fileSize: item.fileSize ?? null,
      sortOrder: item.sortOrder,
    }))

  return {
    property: {
      _id: row.id,
      title: row.name,
      listingId: row.listingIdentifier,
      standardStatus: statusLabel(row.status),
      propertyType: row.propertyType,
      listPrice: row.listPrice,
      city: row.city,
      stateOrProvince: row.stateOrProvince,
      neighborhood: row.neighborhood,
      latitude: row.latitude,
      longitude: row.longitude,
      bedroomsTotal: row.bedrooms,
      bathroomsFull: row.bathroomsFull,
      bathroomsHalf: row.bathroomsHalf,
      bathroomsTotal: row.bathrooms,
      livingArea: row.squareFeet,
      lotSizeArea: row.lotSize,
      lotSizeUnits: row.lotSizeUnits,
      lotSizeSqft: row.lotSizeSqft,
      roadFrontageFeet: row.roadFrontageFeet,
      roadSurfaceType: row.roadSurfaceType,
      lotDescription: row.lotDescription,
      utilitiesNotes: row.utilitiesNotes,
      yearBuilt: row.yearBuilt,
      stories: row.stories,
      parkingSpaces: row.parkingSpaces,
      viewType: row.viewType,
      waterAccess: row.waterAccess,
      beachAccess: row.beachAccess,
      amenities: row.amenities,
      shortDescription: row.shortDescription,
      editorialDescription: row.editorialDescription,
      architecture: row.architectureNotes,
      lifestyleTags: row.lifestyleTags,
      listingAgentName: row.listingAgentName,
      listingAgentEmail: row.listingAgentEmail,
      listingAgentPhone: row.listingAgentPhone,
      listingOffice: row.listingOffice,
    },
    heroUrl: row.heroMediaId ? `/api/media/${row.heroMediaId}` : null,
    galleryImages,
    videos,
    documents,
  }
}

export async function getPropertyBySlug(slug: string): Promise<Result<PropertyDetailResult | null>> {
  const operation = 'property.bySlug'
  try {
    const property = await rustApiPublicProperty(slug)
    return { ok: true, data: property ? propertyDetailFromRust(property) : null }
  } catch (error) {
    return failure(operation, error)
  }
}
