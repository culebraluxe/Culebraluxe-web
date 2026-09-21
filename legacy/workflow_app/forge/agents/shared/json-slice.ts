/**
 * Shared JSON extraction for contract markers.
 *
 * WHY THIS EXISTS: a real model reliably produces the OBJECT and unreliably
 * produces the LABEL in front of it. Observed in one evening of live runs:
 * a perfect routing proposal with no `LEAD_ROUTING:` prefix, and an evidence block
 * whose marker was absent. A missing label is a formatting slip, not a missing
 * decision — so every marker parser gets the same fallback: scan for the last
 * balanced object that matches the expected SHAPE, and accept it.
 *
 * The shape check is what keeps this safe: only an object that looks like the
 * contract (right version, right distinctive field) can be picked up, so a marker
 * parser can never mistake one contract's payload for another's.
 */

/** The balanced `{…}` starting at `start`, or null. String-aware. */
export function balancedObjectAt(text: string, start: number): string | null {
  let depth = 0
  let inString = false
  let escaped = false
  for (let i = start; i < text.length; i++) {
    const ch = text[i]
    if (inString) {
      if (escaped) escaped = false
      else if (ch === '\\') escaped = true
      else if (ch === '"') inString = false
      continue
    }
    if (ch === '"') inString = true
    else if (ch === '{') depth++
    else if (ch === '}') {
      depth--
      if (depth === 0) return text.slice(start, i + 1)
    }
  }
  return null
}

/**
 * The LAST balanced object in `text` that parses and satisfies `matches`.
 *
 * Last, not first: a model echoes instructions before answering, so its real
 * output is at the END of the reply. Scanning right-to-left also means an echoed
 * example can never shadow the genuine emission.
 */
export function lastJsonObjectMatching(
  text: string,
  matches: (row: Record<string, unknown>) => boolean,
): Record<string, unknown> | null {
  if (!text) return null
  let at = text.lastIndexOf('{')
  while (at >= 0) {
    const slice = balancedObjectAt(text, at)
    if (slice) {
      try {
        const parsed: unknown = JSON.parse(slice)
        if (parsed && typeof parsed === 'object' && !Array.isArray(parsed)) {
          const row = parsed as Record<string, unknown>
          if (matches(row)) return row
        }
      } catch {
        /* not JSON — keep scanning left */
      }
    }
    // Guard the progress explicitly. `lastIndexOf(x, -1)` clamps `fromIndex` to 0,
    // so a reply that STARTS with `{` would re-find index 0 forever.
    if (at === 0) break
    const next = text.lastIndexOf('{', at - 1)
    if (next >= at) break
    at = next
  }
  return null
}
