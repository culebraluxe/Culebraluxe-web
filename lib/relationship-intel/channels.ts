// ---------------------------------------------------------------------------
// REL-INTEL — the ONE communication-channel vocabulary.
//
// Three places used to disagree about what a channel is called. The CRM pane
// knows call / email / imessage / sms / whatsapp / meeting / showing / note; the
// materialized view emits raw sources like `apple_calls` and `apple_facetime`; and
// the evidence summarizer had its own shorter map that silently DROPPED calls,
// FaceTime and WhatsApp from the per-source projections while still counting them
// in the header totals. So call rows rendered with a generic globe and "active
// sources" understated.
//
// Normalization belongs at the service boundary, not in the view. The raw
// `source` stays the truth in the warehouse; this map is the only place a source
// becomes a channel, so fixing it here fixes every consumer at once.
// ---------------------------------------------------------------------------

export const COMMS_CHANNELS = [
  'call',
  'email',
  'imessage',
  'sms',
  'whatsapp',
  'meeting',
  'showing',
  'note',
  'other',
] as const

export type CommsChannel = (typeof COMMS_CHANNELS)[number]

/** Raw evidence/MV source -> canonical channel. */
const BY_SOURCE: Record<string, CommsChannel> = {
  apple_messages: 'imessage',
  apple_calls: 'call',
  apple_facetime: 'meeting',
  apple_calendar: 'meeting',
  gmail_contacts: 'email',
  gmail: 'email',
  icloud_mail: 'email',
  email: 'email',
  whatsapp: 'whatsapp',
  sms: 'sms',
}

export const CHANNEL_LABELS: Record<CommsChannel, string> = {
  call: 'Call',
  email: 'Email',
  imessage: 'iMessage',
  sms: 'SMS',
  whatsapp: 'WhatsApp',
  meeting: 'Meeting',
  showing: 'Showing',
  note: 'Note',
  other: 'Other',
}

/** The channels the panel renders source rows for, in display order. */
export const PANEL_CHANNELS: readonly CommsChannel[] = [
  'call',
  'email',
  'imessage',
  'sms',
  'whatsapp',
  'meeting',
  'showing',
  'note',
]

function key(source: string): string {
  return String(source ?? '').trim().toLowerCase()
}

/**
 * The channel a raw source belongs to. An unrecognised source becomes 'other'
 * rather than being dropped: a channel we cannot name is still a contact.
 */
export function channelForSource(source: string): CommsChannel {
  return BY_SOURCE[key(source)] ?? 'other'
}

/** True when the source is part of the known vocabulary with a real channel. */
export function isKnownSource(source: string): boolean {
  return Object.prototype.hasOwnProperty.call(BY_SOURCE, key(source))
}

/** Every email source collapses to one presentation node. */
export function presentationSourceFor(source: string): string {
  return channelForSource(source) === 'email' ? 'email' : String(source ?? '')
}
