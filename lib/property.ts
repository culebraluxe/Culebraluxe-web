// Shared formatting helpers for CulebraLuxe property data.
// Honors the property UI contract: hide missing values, "Price Upon Request"
// when no price, and Land listings never show beds/baths.

export type PropertyLike = {
  propertyType?: string | null
  listPrice?: number | null
  bedroomsTotal?: number | null
  bathroomsTotal?: number | null
  bathroomsFull?: number | null
  bathroomsHalf?: number | null
  livingArea?: number | null
  lotSizeArea?: number | null
  lotSizeUnits?: string | null
  viewType?: string[] | null
  neighborhood?: string | null
}

export function isLand(type?: string | null): boolean {
  return (type ?? '').toLowerCase() === 'land'
}

export function formatPrice(listPrice?: number | null): string {
  if (listPrice == null || listPrice <= 0) return 'Price Upon Request'
  return `$${listPrice.toLocaleString('en-US')}`
}

export function formatArea(
  area?: number | null,
  units?: string | null,
): string | null {
  if (area == null) return null
  const isAcre = (units ?? '').toLowerCase() === 'acres'
  if (isAcre) {
    return `${area} ${area === 1 ? 'Acre' : 'Acres'}`
  }
  return `${area.toLocaleString('en-US')} SF`
}

// Compact key facts row used under the title (icons rendered by the caller).
export function factItems(p: PropertyLike): { label: string; value: string }[] {
  const items: { label: string; value: string }[] = []

  if (!isLand(p.propertyType)) {
    if (p.bedroomsTotal != null) {
      items.push({ label: 'Beds', value: String(p.bedroomsTotal) })
    }
    if (p.bathroomsTotal != null) {
      items.push({ label: 'Baths', value: String(p.bathroomsTotal) })
    }
    if (p.livingArea != null) {
      items.push({
        label: 'Interior',
        value: `${p.livingArea.toLocaleString('en-US')} SF`,
      })
    }
  }

  const lot = formatArea(p.lotSizeArea, p.lotSizeUnits)
  if (lot) items.push({ label: 'Lot', value: lot })

  return items
}

// ---------------------------------------------------------------------------
// Property CARD projection (PX-33) — the compact facts displayed on a
// property card. Derived ONLY from real property data; missing data is
// omitted, never fabricated or defaulted. A property with no location is
// rendered without a location fact — never invented as "Culebra, Puerto Rico"
// or any inferred value. Land listings never show beds/baths.
// ---------------------------------------------------------------------------

export type PropertyCardLike = {
  propertyType?: string | null
  listPrice?: number | null
  neighborhood?: string | null
  location?: string | null
  city?: string | null
  bedrooms?: number | null
  bathrooms?: number | null
  squareFeet?: number | null
  lotSize?: number | null
  lotSizeUnits?: string | null
  views?: string[] | null
}

/** Property-card location fact. Returns null when the record has no usable
 *  location (neighborhood / location / city all absent) so callers can omit
 *  it — never a defaulted "Culebra, Puerto Rico". */
export function propertyLocation(p: PropertyCardLike): string | null {
  return p.neighborhood ?? p.location ?? p.city ?? null
}

/** Ordered property-card facts with missing values omitted (not defaulted).
 *  Bedrooms / bathrooms / interior area for residences; lot for Land; plus the
 *  primary view. */
export function propertyFactParts(p: PropertyCardLike): string[] {
  const parts: string[] = []

  if (!isLand(p.propertyType)) {
    if (p.bedrooms != null) parts.push(`${p.bedrooms} Bed`)
    if (p.bathrooms != null) parts.push(`${p.bathrooms} Bath`)
    if (p.squareFeet != null) {
      parts.push(`${p.squareFeet.toLocaleString('en-US')} SF`)
    }
  } else {
    const lot = formatArea(p.lotSize, p.lotSizeUnits)
    if (lot) parts.push(lot)
  }

  if (p.views?.[0]) {
    parts.push(`${p.views[0]} View`)
  }

  return parts
}

/** Compact property-card facts string, joined with " · ". */
export function propertyFacts(p: PropertyCardLike): string {
  return propertyFactParts(p).join(' · ')
}
