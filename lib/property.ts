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
