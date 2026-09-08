export type RegridFields = Record<string, unknown>

export type RegridGeometry = {
  type?: string
  coordinates?: unknown
  [key: string]: unknown
} | null

export type RegridParcelFeature = {
  id?: string | number | null
  type?: string
  geometry?: RegridGeometry
  properties?: {
    headline?: string | null
    path?: string | null
    ll_uuid?: string | null
    fields?: RegridFields
    [key: string]: unknown
  }
  [key: string]: unknown
}

export type RegridParcel = {
  llUuid: string | null
  parcelNumber: string | null
  path: string | null
  headline: string | null
  situsAddress: string | null
  owner: string | null
  urbanization: string | null
  neighborhood: string | null
  zoning: string | null
  zoningDescription: string | null
  legalDescription: string | null
  latitude: number | null
  longitude: number | null
  lotAcres: number | null
  lotSquareFeet: number | null
  buildingSquareFeet: number | null
  yearBuilt: number | null
  stories: number | null
  bedrooms: number | null
  bathrooms: number | null
  parcelValue: number | null
  landValue: number | null
  improvementValue: number | null
  fields: RegridFields
  geometry: RegridGeometry
  rawFeature: RegridParcelFeature
}

export type RegridAddressLookupInput = {
  query: string
  /** Regrid geographic path. CulebraLuxe defaults to Puerto Rico (/us/pr). */
  path?: string | null
  /** We use 2 by default so a single HTTP call can distinguish 1 match from ambiguity. */
  limit?: number
}

export type RegridAddressLookupResult =
  | {
      status: 'not_found'
      query: string
      path: string | null
      matches: []
    }
  | {
      status: 'matched'
      query: string
      path: string | null
      parcel: RegridParcel
      matches: [RegridParcel]
    }
  | {
      status: 'ambiguous'
      query: string
      path: string | null
      matches: RegridParcel[]
    }

export type RegridClientConfig = {
  token: string
  /** Defaults to https://app.regrid.com/api/v2. */
  baseUrl?: string
  /** Defaults to /us/pr. Override with REGRID_PATH when a narrower path is known. */
  defaultPath?: string | null
  timeoutMs?: number
  fetchImpl?: typeof fetch
}

export interface RegridAddressClient {
  lookupAddress(input: RegridAddressLookupInput): Promise<RegridAddressLookupResult>
}

export type RegridPropertyTarget = {
  id: string
  addressLine1: string | null
  city: string | null
  stateOrProvince: string | null
  postalCode: string | null
  country: string | null
  isoCountryCode: string | null
  catastroNumber: string | null
}

export type RegridPropertyEnrichmentWrite = {
  propertyId: string
  lookupQuery: string
  parcel: RegridParcel
}

export type RegridPropertyEnrichmentReceipt = {
  propertyId: string
  catastroNumber: string | null
  catastroSource: string | null
  regridParcelNumber: string | null
  regridLlUuid: string | null
  regridEnrichedAt: unknown
}

export interface RegridPropertyEnrichmentRepository {
  getTarget(propertyId: string): Promise<RegridPropertyTarget | null>
  persist(input: RegridPropertyEnrichmentWrite): Promise<RegridPropertyEnrichmentReceipt>
}

export type RegridPropertyEnrichmentResult =
  | { status: 'not_found'; propertyId: string; query: string }
  | { status: 'ambiguous'; propertyId: string; query: string; matches: RegridParcel[] }
  | {
      status: 'enriched'
      propertyId: string
      query: string
      parcel: RegridParcel
      receipt: RegridPropertyEnrichmentReceipt
    }
