// ---------------------------------------------------------------------------
// REL-INTEL — display-name classification.
//
// CORE RULE: IDENTITY IS NOT DISPLAY NAME. A phone number or email stored in
// person_identity is never a good client name. `isHumanName` distinguishes a
// real human name from an identity string used as an (unresolved) fallback, so
// enrichment can replace the fallback with a trusted contact name and mark the
// rest as unresolved instead of presenting the identity string as the name.
// ---------------------------------------------------------------------------

/**
 * True when the value looks like a real human name (letters present, not an
 * email, not a phone-like identity string). Used to decide whether a canonical
 * Person's display_name is a good name or an unresolved identity fallback.
 */
export function isHumanName(value: string | null | undefined): boolean {
  const v = (value ?? "").trim()
  if (!v) return false
  if (v.includes("@")) return false
  // Structured identifiers (e.g. "…:ABPerson") are not human names.
  if (v.includes(":")) return false
  // Phone-like: digits and phone punctuation only.
  if (/^[+0-9\s().-]+$/.test(v)) return false
  return true
}
