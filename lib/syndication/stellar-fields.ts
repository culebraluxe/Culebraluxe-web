import type { ListingSource, StellarDetails } from './types'

export const STELLAR_DETAIL_FIELDS = [
  ['listingContractDate', 'Listing contract date', 'date'],
  ['expirationDate', 'Expiration date', 'date'],
  ['listingType', 'Listing type / service', 'text'],
  ['agentMlsId', 'Listing agent MLS ID', 'text'],
  ['taxId', 'Tax / parcel ID', 'text'],
  ['taxYear', 'Tax year', 'number'],
  ['annualTax', 'Annual property tax (USD)', 'number'],
  ['legalDescription', 'Legal description', 'text'],
  ['zoning', 'Zoning', 'text'],
  ['totalAreaSqft', 'Total area (sq ft)', 'number'],
  ['heatedAreaSource', 'Heated area source', 'text'],
  ['ownershipType', 'Ownership type', 'text'],
  ['hoaDetails', 'HOA details', 'text'],
  ['showingInstructions', 'Showing instructions (private)', 'text'],
  ['occupantType', 'Occupant type', 'text'],
] as const satisfies readonly (readonly [keyof StellarDetails, string, string])[]

export function stellarMapping(source: ListingSource) {
  const direct = {
    ListPrice: source.listPrice,
    StreetNumber: source.streetNumber,
    StreetName: source.streetName,
    UnitNumber: source.unitNumber,
    City: source.city,
    StateOrProvince: source.stateOrProvince,
    PostalCode: source.postalCode,
    BedroomsTotal: source.bedrooms,
    YearBuilt: source.yearBuilt,
    PublicRemarks: source.publicRemarks,
  }
  // Values requiring a unit check, vocabulary selection or human review.
  const converted = {
    PropertyStyle: source.propertyType,
    BathroomsFull: source.bathroomsFull,
    BathroomsHalf: source.bathroomsHalf,
    // HEATED AREA IS THE INTERIOR AREA HERE, and this is a DECISION rather than an assumption —
    // confirmed by the captain 2026-09-18. In a northern market "heated" excludes unconditioned
    // space such as a covered porch, which makes property.square_feet an ambiguous source for an MLS
    // `HeatedArea` field. On Culebra the interior square footage IS the heated area, so the property's
    // own measurement is the right value and it is NOT flagged for review. The semantics are also
    // recorded on the column itself (migration 195) so a schema reader meets them without asking.
    HeatedAreaSqFt: source.squareFeet,
    // A lot size WITHOUT units is not a usable Stellar value, and the packet lists "lot units" as an
    // explicit review item — so a unitless number counts as missing rather than as a filled field.
    // The fence caught this: the code only flagged a lot size that was absent entirely.
    LotSize:
      source.lotSize != null && source.lotSizeUnits
        ? { value: source.lotSize, units: source.lotSizeUnits }
        : null,
    ListingAgent: source.listingAgentName,
    Photos: source.photos.length ? source.photos.map((photo) => photo.url) : source.imageCount ? `${source.imageCount} images in Property Media` : null,
  }
  const additional = Object.fromEntries(
    STELLAR_DETAIL_FIELDS.map(([key, label]) => [label, source.stellar?.[key] ?? null]),
  )
  const missing = [
    ...Object.entries(direct).filter(([, value]) => value == null || value === '').map(([key]) => key),
    ...Object.entries(converted).filter(([, value]) => value == null || value === '').map(([key]) => `${key} (review)`),
    ...Object.entries(additional).filter(([, value]) => value == null || value === '').map(([key]) => key),
  ]
  return {
    direct,
    converted,
    additional,
    missing,
    // DERIVED, NOT ASSERTED. These were literals (10/7/15), which reads as a promise nothing checks:
    // adding a field to any map would leave the form claiming the old count, and the packet's whole
    // acceptance is about those counts. Derived from the maps, they cannot drift, and the fence
    // asserts them against Object.keys lengths as a drift guard.
    counts: {
      direct: Object.keys(direct).length,
      converted: Object.keys(converted).length,
      additional: Object.keys(additional).length,
    },
  }
}

/**
 * The refusal rules, PURE and OUTSIDE the server action.
 *
 * They used to live inside `saveStellarDetailsActionHandler`, which meant the packet's acceptance
 * bullet "invalid dates, negative tax and invalid tax year are refused" could only be exercised by
 * driving a Next request against a live database — so it was unprovable in the factory and invisible
 * to the form's own tests. Same rules, same messages, now reachable by a fence.
 */
export function validateStellarDetails(
  raw: Record<string, string | null | undefined>,
): { ok: true; values: StellarDetails } | { ok: false; error: string } {
  const values = {} as StellarDetails
  for (const [key, , kind] of STELLAR_DETAIL_FIELDS) {
    const text = String(raw[key] ?? '').trim()
    if (text.length > 4000) return { ok: false, error: `${key} is too long.` }
    if (kind === 'number') {
      const value = text ? Number(text) : null
      if (
        value !== null &&
        (!Number.isFinite(value) ||
          value < 0 ||
          (key === 'taxYear' && (!Number.isInteger(value) || value < 1800 || value > 2200)) ||
          (key === 'totalAreaSqft' && value === 0))
      ) {
        return { ok: false, error: `Check ${key}.` }
      }
      Object.assign(values, { [key]: value })
    } else {
      if (kind === 'date' && text && !/^\d{4}-\d{2}-\d{2}$/.test(text)) {
        return { ok: false, error: `Check ${key}.` }
      }
      Object.assign(values, { [key]: text || null })
    }
  }
  if (
    values.listingContractDate &&
    values.expirationDate &&
    values.expirationDate < values.listingContractDate
  ) {
    return { ok: false, error: 'Expiration must be after the contract date.' }
  }
  return { ok: true, values }
}
