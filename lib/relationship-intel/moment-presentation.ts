// ---------------------------------------------------------------------------
// REL-INTEL — Contact History presentation helpers (pure, testable).
//
// The Cloze-style timeline renders relationship memory, not event metadata:
// human relationship direction (Ana → Lisa / Lisa → Ana / Ana ↔ Lisa) instead
// of generic INBOUND/OUTBOUND, and a single channel label (never "Email / Email"
// duplication). Lisa is the owner/operator identity.
// ---------------------------------------------------------------------------

/** The CulebraLuxe owner/operator identity shown on the client-facing timeline. */
export const OWNER_NAME = 'Lisa'

export type MomentDirection = 'inbound' | 'outbound' | 'two-way' | null

/** Human relationship direction for a timeline row. */
export function humanDirection(
  direction: MomentDirection,
  clientName: string,
): string | null {
  const c = clientName.trim() || 'Client'
  switch (direction) {
    case 'inbound':
      return `${c} → ${OWNER_NAME}`
    case 'outbound':
      return `${OWNER_NAME} → ${c}`
    case 'two-way':
      return `${c} ↔ ${OWNER_NAME}`
    default:
      return null
  }
}

/** Generic single-event direction for the header summary line. */
export function headerDirectionLabel(
  dir: 'inbound' | 'outbound' | null,
): 'Inbound' | 'Outbound' | null {
  if (dir === 'outbound') return 'Outbound'
  if (dir === 'inbound') return 'Inbound'
  return null
}

/**
 * Channel / conversation description line. A burst is described as
 * "iMessage conversation · 47 messages"; a single event is just the channel
 * label (no invented second line, no "Email / Email" duplication).
 */
export function channelLine(label: string, isBurst: boolean, count: number): string {
  if (!isBurst) return label
  return `${label} conversation · ${count} message${count === 1 ? '' : 's'}`
}

/**
 * Sanitize a stored preview for display: strip control / object-replacement
 * characters (Apple stores U+FFFC for attachments/emoji in the text column) so
 * a cue reads as memory, not broken metadata. Returns null when nothing
 * human-readable remains (the memory-cue line is then omitted).
 */
export function cleanPreview(preview: string | null): string | null {
  if (!preview) return null
  const cleaned = preview
    .replace(/[\u0000-\u001f\u007f\ufeff\ufffc\ufffd]+/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  return cleaned.length > 0 ? cleaned : null
}

/**
 * Keep the content, timestamp, and direction from the same relationship
 * moment. Overall source activity can be newer than the latest useful preview
 * (for example, an Apple attachment placeholder after the last text message).
 */
export function sourceContextMoment(input: {
  lastContext: string | null
  lastContextAt: string | null
  lastContextDirection: 'inbound' | 'outbound' | null
  lastContactAt: string | null
  lastDirection: 'inbound' | 'outbound' | null
}): {
  preview: string | null
  timestamp: string | null
  direction: 'inbound' | 'outbound' | null
} {
  const preview = cleanPreview(input.lastContext)
  if (!preview) {
    return {
      preview: null,
      timestamp: input.lastContactAt,
      direction: input.lastDirection,
    }
  }
  return {
    preview,
    timestamp: input.lastContextAt ?? input.lastContactAt,
    direction: input.lastContextDirection ?? input.lastDirection,
  }
}
