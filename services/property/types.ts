import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'
import type { Result } from '@/db/client'
import type { PropertyDetailResult } from '@/lib/property-types'

/**
 * Canonical reusable address/place DTO.
 *
 * Address-shaped facts live in Property. Apple Contacts, Forms, Client Lens,
 * and Contracts can all use this shape without learning one another's storage.
 */
export type PropertyAddressDto = {
  addressLine1: string | null
  city: string | null
  stateOrProvince: string | null
  neighborhood: string | null
  postalCode: string | null
  country: string | null
  isoCountryCode: string | null
}

/** Why a Property matters to a Person. Property still owns the place truth. */
export type PersonPropertyRelation =
  | 'address'
  | 'legal_address'
  | 'physical_property'
  | 'interest'

export type PropertyDto = {
  id: string
  /** Presentation fallback retained for existing callers. */
  displayName: string
  /** Human/local name for the place, e.g. "Casa Luar" or "Sea to Soul". */
  localName: string | null
  /** Name appearing on title, e.g. a person, LLC, or trust. Not a Person identity. */
  legalOwnerName: string | null
  /** Existing Property title/listing identifier used by LISTING-01 as catastro. */
  catastroNumber?: string | null
  /** Puerto Rico registry facts proved intrinsic by PR-PNS. */
  registryEntry?: string | null
  fincaNumber?: string | null
  registrySection?: string | null
  /** Compatibility aliases for the first service-core experiment. */
  addressLine1: string | null
  municipality: string | null
  /** Canonical structured address for UI/Form/Contract reuse. */
  address: PropertyAddressDto
  status: string
  archivedAt: string | null
}

/** Contextual relationship to a canonical Property; Property identity stays independent. */
export type PropertyForPersonDto = {
  relation: PersonPropertyRelation
  relationStatus: string | null
  property: PropertyDto
}

export type PersonPropertyContextDto = {
  personId: string
  properties: PropertyForPersonDto[]
}

/* ------------------------------------------------------------------ *
 * PUBLIC INVENTORY READS
 *
 * The public site (home, buyers, listing page, favorites) reads inventory
 * through these. They answer one question: which properties are ACTIVE
 * LISTINGS we are marketing right now? A property that is only a known
 * place (for example an Apple Contacts address) is not inventory and never
 * appears here.
 * ------------------------------------------------------------------ */

/** Compact inventory card used by every public surface. */
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

/** Minimal property reference for intros and cross-links. */
export type PropertyIntro = {
  id: string
  name: string
  location: string | null
}

/** Buyers search contract (PX-24B): structured filters applied in SQL. */
export type PropertyFilterInput = {
  category?: 'all' | 'homes' | 'land'
  q?: string
  maxPrice?: number | null
  beds?: number | null
  view?: string
  sort?: 'featured' | 'price-high' | 'price-low' | 'name'
}

export type ListPropertiesRequest = {
  /**
   * true  -> ACTIVE LISTINGS only (public surfaces MUST pass this).
   * false -> the working lifecycle set used by internal portal pickers.
   */
  publicOnly?: boolean
}

export type FilterPropertiesRequest = { filters: PropertyFilterInput }

export type GetPropertyBySlugRequest = { slug: string }

export type GetSimilarPropertiesRequest = {
  propertyId: string
  current: {
    propertyType: string | null
    city: string | null
    neighborhood: string | null
    listPrice: number | null
  }
  limit?: number
}

export type ListPropertySlugsRequest = Record<string, never>
export type GetPropertyIntroRequest = { propertyId: string }

export type PropertyInventoryPage = {
  properties: PropertySummary[]
  viewOptions: string[]
}

export type GetPropertyRequest = { propertyId: string }
export type FindPropertyByAddressRequest = {
  addressLine1: string
  municipality?: string
  stateOrProvince?: string
  postalCode?: string
}
export type GetPropertiesForPersonRequest = { personId: string }

/**
 * Canonical two-way ingress used by Forms and later ODS promotion.
 * Undefined means "leave unchanged"; null explicitly clears an optional qualifier.
 */
export type UpsertPropertyForPersonRequest = {
  personId: string
  relation: PersonPropertyRelation
  propertyId?: string
  relationStatus?: string | null
  address?: Partial<PropertyAddressDto>
  localName?: string | null
  legalOwnerName?: string | null
  catastroNumber?: string | null
  registryEntry?: string | null
  fincaNumber?: string | null
  registrySection?: string | null
  sourceType?: string
  sourceKey?: string | null
}

export type SetPropertyDisplayNameRequest = { propertyId: string; displayName: string }
export type SetPropertyStatusRequest = { propertyId: string; status: string }

export const PROPERTY_OPERATIONS = {
  GET: 'property.get',
  FIND_BY_ADDRESS: 'property.findByAddress',
  FOR_PERSON: 'property.forPerson',
  UPSERT_FOR_PERSON: 'property.upsertForPerson',
  SET_DISPLAY_NAME: 'property.setDisplayName',
  SET_STATUS: 'property.setStatus',
  LIST: 'property.list',
  SEARCH: 'property.search',
  SIMILAR: 'property.similar',
  BY_SLUG: 'property.bySlug',
  PUBLIC_SLUGS: 'property.publicSlugs',
  INTRO: 'property.intro',
} as const

export type PropertyOperationMap = {
  'property.get': { request: GetPropertyRequest; response: PropertyDto | null }
  'property.findByAddress': { request: FindPropertyByAddressRequest; response: PropertyDto | null }
  'property.forPerson': { request: GetPropertiesForPersonRequest; response: PersonPropertyContextDto }
  'property.upsertForPerson': { request: UpsertPropertyForPersonRequest; response: PropertyForPersonDto }
  'property.setDisplayName': { request: SetPropertyDisplayNameRequest; response: PropertyDto }
  'property.setStatus': { request: SetPropertyStatusRequest; response: PropertyDto }
  // Public inventory reads. They keep the DB-HARDEN-01C contract (a failed read is
  // a Result, never a throw) so one broken source degrades a page instead of
  // rejecting it. Result is the database gateway's shared type — type-only import.
  'property.list': { request: ListPropertiesRequest; response: Result<PropertySummary[]> }
  'property.search': { request: FilterPropertiesRequest; response: Result<PropertyInventoryPage> }
  'property.similar': { request: GetSimilarPropertiesRequest; response: Result<PropertySummary[]> }
  'property.bySlug': { request: GetPropertyBySlugRequest; response: Result<PropertyDetailResult | null> }
  'property.publicSlugs': { request: ListPropertySlugsRequest; response: Result<string[]> }
  'property.intro': { request: GetPropertyIntroRequest; response: Result<PropertyIntro | null> }
}

export type PropertyOperationName = ServiceOperationName<PropertyOperationMap>
export type PropertyEnvelope<K extends PropertyOperationName = PropertyOperationName> =
  ServiceEnvelopeFor<PropertyOperationMap, K>
