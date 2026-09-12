// ---------------------------------------------------------------------------
// REL-INTEL — the communication vocabularies the CRM Client pane actually renders.
//
// THE SCREEN IS THE CONTRACT. components/portal/contact-history.tsx declares two
// different vocabularies for two different jobs, and they are NOT interchangeable:
//
//   SOURCE_SLOTS   the six source rows: Phone, iMessage, WhatsApp, Email,
//                  FaceTime, Apple Calendar. Phone deliberately EXCLUDES FaceTime
//                  (`!sourceIncludes(channel, 'facetime')`): a call and a FaceTime
//                  are different things and the pane shows them as different rows.
//
//   channelMeta    the individual MOMENT rows on the timeline: call, email,
//                  imessage, sms, whatsapp, meeting, showing, note. Those come from
//                  interaction.channel, which is already canonical.
//
// So a SOURCE maps to a source-row channel, and a MOMENT carries a moment channel.
// Deriving one from the other is what put apple_calls and apple_facetime outside the
// pane's vocabulary and made both render with a generic globe.
// ---------------------------------------------------------------------------

/** The source-row vocabulary. Mirrors SOURCE_SLOTS in the pane. */
export const COMMS_SOURCE_CHANNELS = [
  'call',
  'facetime',
  'imessage',
  'whatsapp',
  'email',
  'calendar',
] as const

export type CommsSourceChannel = (typeof COMMS_SOURCE_CHANNELS)[number] | 'other'

export const SOURCE_CHANNEL_LABELS: Record<CommsSourceChannel, string> = {
  call: 'Phone',
  facetime: 'FaceTime',
  imessage: 'iMessage',
  whatsapp: 'WhatsApp',
  email: 'Email',
  calendar: 'Apple Calendar',
  other: 'Other',
}

/** How many source rows the pane can show — the "X of N" denominator. */
export const COMMS_SOURCE_SLOT_COUNT = COMMS_SOURCE_CHANNELS.length

/** The moment vocabulary. Mirrors channelMeta in the pane. */
export const COMMS_MOMENT_CHANNELS = [
  'call',
  // A FaceTime is a distinct moment, not a phone call. The pane's channelMeta
  // needs this entry too, or the fact is lost between warehouse and screen.
  'facetime',
  'email',
  'imessage',
  'sms',
  'whatsapp',
  'meeting',
  'showing',
  'note',
] as const

export type CommsMomentChannel = (typeof COMMS_MOMENT_CHANNELS)[number]

const MOMENT_LABELS: Record<CommsMomentChannel, string> = {
  call: 'Call',
  facetime: 'FaceTime',
  email: 'Email',
  imessage: 'iMessage',
  sms: 'SMS',
  whatsapp: 'WhatsApp',
  meeting: 'Meeting',
  showing: 'Showing',
  note: 'Note',
}

/**
 * The intake already delineated a video call from a phone call, and the warehouse
 * preserved it: scripts/apple-calls-intake.ts writes source_system
 * 'apple_facetime' and event_type 'facetime_call' for a FaceTime, and 'apple_calls'
 * / 'phone_call' otherwise (lib/relationship-intel/apple-calls.ts isFaceTimeCall,
 * driven by the provider/call type Apple reports). Both are stored with
 * channel 'call', so the distinction has to be read back from those fields rather
 * than inferred from the channel. Measured in PROD: 4,661 phone_call rows and 85
 * facetime_call rows.
 */
export function isFaceTimeInteraction(input: {
  sourceSystem?: string | null
  eventType?: string | null
}): boolean {
  const source = key(input.sourceSystem ?? '')
  const event = key(input.eventType ?? '')
  return source === 'apple_facetime' || event === 'facetime_call'
}

/**
 * Raw warehouse source -> source-row channel. `apple_calls` and `apple_facetime`
 * stay DISTINCT: the pane renders Phone and FaceTime as separate rows.
 */
const SOURCE_BY_NAME: Record<string, CommsSourceChannel> = {
  apple_messages: 'imessage',
  apple_calls: 'call',
  apple_facetime: 'facetime',
  apple_calendar: 'calendar',
  calendar: 'calendar',
  eventkit: 'calendar',
  gmail_contacts: 'email',
  gmail: 'email',
  icloud_mail: 'email',
  email: 'email',
  whatsapp: 'whatsapp',
}

function key(value: string): string {
  return String(value ?? '').trim().toLowerCase()
}

/** The source row a raw source belongs to. Unknown sources are 'other', never dropped. */
export function sourceChannelFor(source: string): CommsSourceChannel {
  return SOURCE_BY_NAME[key(source)] ?? 'other'
}

export function sourceChannelLabel(channel: CommsSourceChannel): string {
  return SOURCE_CHANNEL_LABELS[channel]
}

/** True when the source is one the pane has a slot for. */
export function isKnownSource(source: string): boolean {
  return Object.prototype.hasOwnProperty.call(SOURCE_BY_NAME, key(source))
}

/** The moment channel for a canonical interaction, or null when it is not one we render. */
export function momentChannelFor(value: string | null | undefined): CommsMomentChannel | null {
  const lowered = key(value ?? '')
  return (COMMS_MOMENT_CHANNELS as readonly string[]).includes(lowered)
    ? (lowered as CommsMomentChannel)
    : null
}

export function momentChannelLabel(channel: CommsMomentChannel): string {
  return MOMENT_LABELS[channel]
}
