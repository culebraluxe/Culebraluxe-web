// ---------------------------------------------------------------------------
// OPS-03 — Property Administration: pure write contract for the property CRUD
// seam. No database imports; any layer can import and test it.
//
// One place for the property status vocabularies and the create-input
// normalization/validation shared by the server actions and the db seam
// (defense in depth: the action layer and the db seam both normalize through
// here, so a bypassed action can never write a malformed property row).
// Validation failures throw PortalWriteError('validation') so the portal
// write action maps them to a friendly result without classifying by message
// text. The numeric/slug rules mirror the listing-facts writer
// (db/portal-property.ts) so a freshly created property always satisfies the
// same constraints the facts editor enforces.
// ---------------------------------------------------------------------------

import { PortalWriteError } from './portal-write-error'

// Statuses an operator may set from listing administration (create / facts /
// visibility). Transaction states (under contract, sold) are owned by the
// CRM-14 transaction workflow and are never writable from administration.
export const EDITABLE_PROPERTY_STATUSES = [
  'prospect',
  'coming_soon',
  'active',
  'off_market',
] as const
export type EditablePropertyStatus = (typeof EDITABLE_PROPERTY_STATUSES)[number]

// Full schema vocabulary (001_initial_schema.sql CHECK). 'archived' is the
// status value paired with archived_at; archive() drives archived_at directly
// and the admin table derives "Archived" from that flag.
export const PROPERTY_STATUSES = [
  ...EDITABLE_PROPERTY_STATUSES,
  'under_contract',
  'sold',
  'archived',
] as const
export type PropertyStatus = (typeof PROPERTY_STATUSES)[number]

export const EDITABLE_PROPERTY_STATUS_SET: ReadonlySet<string> = new Set(
  EDITABLE_PROPERTY_STATUSES,
)
export const PROPERTY_STATUS_SET: ReadonlySet<string> = new Set(
  PROPERTY_STATUSES,
)

export function isEditablePropertyStatus(
  value: string,
): value is EditablePropertyStatus {
  return EDITABLE_PROPERTY_STATUS_SET.has(value)
}

export function isPropertyStatus(value: string): value is PropertyStatus {
  return PROPERTY_STATUS_SET.has(value)
}

/**
 * Minimal create surface for the Property Administration screen. Seeded with
 * the operational identity fields from the Portal UI contract (name, location,
 * status, list price, beds/baths/area, property type); the per-listing
 * workspace (facts form) owns the long tail (descriptions, coordinates, lot
 * details, listing agent, media).
 */
export type PropertyCreateInput = {
  name: string
  slug?: string | null
  status?: EditablePropertyStatus
  featured?: boolean
  propertyType?: string | null
  listPrice?: number | null
  location?: string | null
  city?: string | null
  stateOrProvince?: string | null
  neighborhood?: string | null
  bedrooms?: number | null
  bathrooms?: number | null
  squareFeet?: number | null
}

export type NormalizedPropertyCreate = {
  name: string
  slug: string | null
  status: EditablePropertyStatus
  featured: boolean
  propertyType: string | null
  listPrice: number | null
  location: string | null
  city: string | null
  stateOrProvince: string | null
  neighborhood: string | null
  bedrooms: number | null
  bathrooms: number | null
  squareFeet: number | null
}

export const SLUG_PATTERN = /^[a-z0-9]+(?:-[a-z0-9]+)*$/

export function normalizePropertySlug(value: string | null | undefined) {
  const slug = value?.trim() || null
  if (slug && !SLUG_PATTERN.test(slug)) {
    throw new PortalWriteError(
      'validation',
      'Slug must be lowercase letters, numbers, and hyphens.',
    )
  }
  return slug
}

function cleanText(value: string | null | undefined) {
  if (value == null) return null
  const trimmed = value.trim()
  return trimmed.length > 0 ? trimmed : null
}

function nonNegativeNumber(value: number | null | undefined, label: string) {
  if (value == null) return null
  if (!Number.isFinite(value) || value < 0) {
    throw new PortalWriteError('validation', `${label} must be zero or greater.`)
  }
  return value
}

/**
 * Normalize + validate a create-property input. Throws PortalWriteError on
 * any validation failure; returns the fully typed row shape the db seam
 * inserts. `status` defaults to 'prospect' (the schema default) and may only
 * be an editable status — transaction statuses are CRM-14-owned.
 */
export function normalizePropertyCreateInput(
  input: PropertyCreateInput,
): NormalizedPropertyCreate {
  if (!input || typeof input !== 'object') {
    throw new PortalWriteError('validation', 'Property input is invalid.')
  }
  if (!input.name || !input.name.trim()) {
    throw new PortalWriteError('validation', 'Property name is required.')
  }

  const status = input.status ?? 'prospect'
  if (!isEditablePropertyStatus(status)) {
    throw new PortalWriteError(
      'validation',
      'Property status is invalid for listing administration.',
    )
  }

  return {
    name: input.name.trim(),
    slug: normalizePropertySlug(input.slug),
    status,
    featured: input.featured === true,
    propertyType: cleanText(input.propertyType),
    listPrice: nonNegativeNumber(input.listPrice, 'List price'),
    location: cleanText(input.location),
    city: cleanText(input.city),
    stateOrProvince: cleanText(input.stateOrProvince),
    neighborhood: cleanText(input.neighborhood),
    bedrooms: nonNegativeNumber(input.bedrooms, 'Bedrooms'),
    bathrooms: nonNegativeNumber(input.bathrooms, 'Bathrooms'),
    squareFeet: nonNegativeNumber(input.squareFeet, 'Square footage'),
  }
}
