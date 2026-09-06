export const LISTING_CANONICAL_FIELD_NAMES = [
  'sellerName',
  'sellerResidenceAddress',
  'property',
  'propertyLocation',
  'legalOwnerName',
  'catastroNumber',
] as const

export type ListingCanonicalFieldName = typeof LISTING_CANONICAL_FIELD_NAMES[number]
export type ListingCanonicalFields = Record<ListingCanonicalFieldName, string>
export type ListingFieldOrigin = 'person' | 'property' | 'listing_form' | 'empty'

/**
 * Canonical-first read model for the six LISTING-01 fields that belong to
 * Person/Property. Contract-only fields intentionally stay outside this seam.
 */
export type ListingCanonicalSnapshot = {
  personId: string
  personDisplayName: string
  formInstanceId: string | null
  formUpdatedAt: string | null
  legalAddressPropertyId: string | null
  physicalPropertyId: string | null
  fields: ListingCanonicalFields
  origins: Record<ListingCanonicalFieldName, ListingFieldOrigin>
}

export type SaveListingCanonicalFieldsRequest = {
  personId: string
  fields: ListingCanonicalFields
  /** Explicit Form Lens selection may promote an existing related Property. */
  physicalPropertyId?: string | null
}

export const EMPTY_LISTING_CANONICAL_FIELDS: ListingCanonicalFields = {
  sellerName: '',
  sellerResidenceAddress: '',
  property: '',
  propertyLocation: '',
  legalOwnerName: '',
  catastroNumber: '',
}
