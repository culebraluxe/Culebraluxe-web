// ---------------------------------------------------------------------------
// REL-INTEL — deterministic normalization + replay fingerprinting.
//
// - Email: trim + lowercase for matching; original value is always retained;
//   structurally invalid values are quarantined, never silently deleted.
// - Phone: digits-only normalization for US/Puerto Rico numbering; only returns
//   a matchable value when normalization is reliable (10-digit US/PR, or an
//   11-digit value with a leading country "1"). Ambiguous international numbers
//   are quarantined rather than guessed at.
// - Spreadsheet formula injection is neutralized when parsing CSV cells.
// - Fingerprint: stable deterministic (non-crypto) hash used for replay/dedup.
// ---------------------------------------------------------------------------

export type NormalizeResult<T> =
  | { ok: true; value: T }
  | { ok: false; reason: string; original: string }

const EMAIL_RE = /^[^\s@]+@[^\s@]+\.[^\s@]+$/

/** Normalize an email for matching. Original value is preserved by the caller. */
export function normalizeEmail(input: string): NormalizeResult<string> {
  const trimmed = input.trim()
  if (!trimmed) return { ok: false, reason: 'empty', original: input }
  if (trimmed.length > 320) return { ok: false, reason: 'too_long', original: input }
  if (!EMAIL_RE.test(trimmed)) return { ok: false, reason: 'invalid_format', original: input }
  return { ok: true, value: trimmed.toLowerCase() }
}

/**
 * Normalize a phone number for matching (US/Puerto Rico, 10-digit reliable).
 * Returns `ok:false` with a clear reason when normalization is not reliable so
 * the value is quarantined instead of producing a false match.
 */
export function normalizePhone(input: string): NormalizeResult<string> {
  const original = input
  const digits = input.replace(/\D+/g, '')
  if (!digits) return { ok: false, reason: 'empty', original }
  if (digits.length === 11 && digits.startsWith('1')) {
    // 11 digits with leading country code "1" -> drop it (US/PR).
    return { ok: true, value: digits.slice(1) }
  }
  if (digits.length === 10) {
    return { ok: true, value: digits }
  }
  return { ok: false, reason: 'ambiguous_international', original }
}

/** Spreadsheet-formula injection guard: neutralize a leading formula character. */
export function sanitizeSpreadsheetCell(value: string): string {
  const trimmed = value.trim()
  if (!trimmed) return trimmed
  if (/^[=+\-@]/.test(trimmed)) return `'${trimmed}`
  return trimmed
}

/**
 * Stable deterministic fingerprint for replay/dedup. Not a cryptographic hash —
 * it is used only to detect exact replays and payload changes, matching the
 * repository's existing payload_fingerprint convention.
 */
export function fingerprint(input: string): string {
  let h1 = 0xdeadbeef ^ 0
  let h2 = 0x41c6ce57 ^ 0
  for (let i = 0; i < input.length; i++) {
    const ch = input.charCodeAt(i)
    h1 = Math.imul(h1 ^ ch, 2654435761)
    h2 = Math.imul(h2 ^ ch, 1597334677)
  }
  h1 =
    Math.imul(h1 ^ (h1 >>> 16), 2246822507) ^
    Math.imul(h2 ^ (h2 >>> 13), 3266489909)
  h2 =
    Math.imul(h2 ^ (h2 >>> 16), 2246822507) ^
    Math.imul(h1 ^ (h1 >>> 13), 3266489909)
  return (
    (h2 >>> 0).toString(16).padStart(8, '0') +
    (h1 >>> 0).toString(16).padStart(8, '0')
  )
}
