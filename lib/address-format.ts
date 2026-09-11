/**
 * Shared address text normalization for the repository boundary and for the
 * display composers that turn an address DTO into one line of text.
 *
 * Why this exists:
 *
 * A street value from Apple Contacts can legitimately carry more than one line
 * (for example "Bo. Delicias\nVagabundo Capital LLC", where the bo./barrio sits
 * above the street or the owner entity). That is correct source data, and the
 * projection preserves it — the newline is real.
 *
 * The problem is downstream: a single-line form field or label renders "\n" as
 * nothing, so "Bo. Delicias" and "Vagabundo Capital LLC" silently glue together
 * into "Bo. DeliciasVagabundo Capital LLC". Every consumer that asks for a label
 * must therefore receive single-line text. That normalization belongs here, once,
 * rather than in each UI that renders an address.
 */

/**
 * Collapse a possibly multi-line value into one line. Newlines become ", " so
 * distinct address lines stay readable, and runs of whitespace collapse.
 * Returns null for empty/whitespace-only input.
 */
export function oneLine(value: string | null | undefined): string | null {
  const next = (value ?? '')
    .replace(/\r\n?/g, '\n')
    .split('\n')
    .map((part) => part.replace(/\s+/g, ' ').trim())
    .filter(Boolean)
    .join(', ')
  return next ? next : null
}

/**
 * Puerto Rico writes its state as "PR", so a country of "United States" on a PR
 * address adds nothing to a label or a form field. For any other state the
 * country is meaningful and is kept.
 */
export function isRedundantCountry(
  country: string | null | undefined,
  stateOrProvince: string | null | undefined,
): boolean {
  const state = (stateOrProvince ?? '').trim().toUpperCase()
  if (state !== 'PR') return false
  const value = (country ?? '').trim().toLowerCase().replace(/\./g, '')
  return ['united states', 'united states of america', 'us', 'usa'].includes(value)
}

/** The address shape both the repository and the Listing form binding carry. */
export type AddressLineParts = {
  addressLine1?: string | null
  neighborhood?: string | null
  city?: string | null
  stateOrProvince?: string | null
  postalCode?: string | null
  country?: string | null
}

/**
 * One readable line from an address: the single implementation used by the
 * property repository's label and by the Listing form's address fields. Returns
 * '' when there is nothing to show.
 */
export function formatAddressLine(address: AddressLineParts | null | undefined): string {
  if (!address) return ''
  return [
    oneLine(address.addressLine1),
    oneLine(address.neighborhood),
    oneLine(address.city),
    [oneLine(address.stateOrProvince), oneLine(address.postalCode)].filter(Boolean).join(' ') || null,
    isRedundantCountry(address.country, address.stateOrProvince) ? null : oneLine(address.country),
  ]
    .filter((value): value is string => Boolean(value))
    .join(', ')
}
