export const SYNDICATION_CHANNELS = [
  'culebraluxe',
  'facebook_marketplace',
  'stellar_mls',
  'clasificados',
  'pr_mls',
  'amplia_mls',
  'zillow_fsbo',
  'realtor_com',
  'hubspot',
] as const

export type SyndicationChannel = (typeof SYNDICATION_CHANNELS)[number]

export type PublishMode = 'copy_pack' | 'api' | 'mls' | 'blocked'

export type PlacementStatus =
  | 'draft'
  | 'ready'
  | 'pending_manual'
  | 'live'
  | 'expired'
  | 'failed'
  | 'withdrawn'

export type ChannelDefinition = {
  id: SyndicationChannel
  label: string
  shortLabel: string
  mode: PublishMode
  readiness: 'live' | 'pack' | 'stub' | 'blocked'
  defaultTtlDays: number | null
  notes: string
}

export const CHANNEL_CATALOG: Record<SyndicationChannel, ChannelDefinition> = {
  culebraluxe: {
    id: 'culebraluxe',
    label: 'CulebraLuxe site',
    shortLabel: 'Site',
    mode: 'api',
    readiness: 'live',
    defaultTtlDays: null,
    notes: 'Canonical public listing. Driven by property.is_published.',
  },
  facebook_marketplace: {
    id: 'facebook_marketplace',
    label: 'Facebook Marketplace + catalog',
    shortLabel: 'Facebook',
    mode: 'api',
    readiness: 'pack',
    defaultTtlDays: 30,
    notes:
      'Builds Graph home_listing + Page feed payloads for dry-run. Live POST only when META_* tokens and SYNDICATION_LIVE=true. Page /feed works first; catalog id optional until granted.',
  },
  stellar_mls: {
    id: 'stellar_mls',
    label: 'PRAR + Stellar MLS',
    shortLabel: 'Stellar',
    mode: 'mls',
    readiness: 'pack',
    defaultTtlDays: null,
    notes:
      'No public write API. Adapter emits a RESO Property payload + Matrix distribution checklist. Zillow/Realtor.com ride this feed.',
  },
  clasificados: {
    id: 'clasificados',
    label: 'ClasificadosOnline',
    shortLabel: 'Clasificados',
    mode: 'copy_pack',
    readiness: 'pack',
    defaultTtlDays: 40,
    notes: 'No publisher API. Generate pack, paste once, confirm the live URL.',
  },
  pr_mls: {
    id: 'pr_mls',
    label: 'PR Realtors MLS (legacy id)',
    shortLabel: 'PR MLS',
    mode: 'blocked',
    readiness: 'blocked',
    defaultTtlDays: null,
    notes: 'Use Stellar. This id stays so old ledger rows still validate.',
  },
  amplia_mls: {
    id: 'amplia_mls',
    label: 'Amplia MLS',
    shortLabel: 'Amplia',
    mode: 'mls',
    readiness: 'stub',
    defaultTtlDays: null,
    notes: 'Optional extra. Not the flagship path. Confirm portal feed in writing.',
  },
  zillow_fsbo: {
    id: 'zillow_fsbo',
    label: 'Zillow FSBO',
    shortLabel: 'Zillow FSBO',
    mode: 'copy_pack',
    readiness: 'pack',
    defaultTtlDays: null,
    notes: 'Fallback owner card only. Prefer Stellar syndication onto Zillow.',
  },
  realtor_com: {
    id: 'realtor_com',
    label: 'Realtor.com',
    shortLabel: 'Realtor.com',
    mode: 'blocked',
    readiness: 'blocked',
    defaultTtlDays: null,
    notes: 'MLS-only. Comes from Stellar distribution, not a broker upload.',
  },
  hubspot: {
    id: 'hubspot',
    label: 'HubSpot',
    shortLabel: 'HubSpot',
    mode: 'api',
    readiness: 'stub',
    defaultTtlDays: null,
    notes: 'Builds a listing object payload. Live write only with HUBSPOT_ACCESS_TOKEN + SYNDICATION_LIVE.',
  },
}

export function isSyndicationChannel(value: string): value is SyndicationChannel {
  return (SYNDICATION_CHANNELS as readonly string[]).includes(value)
}

// ---------------------------------------------------------------------------
// Channel honesty (V3 §1). Only channels CulebraLuxe can actually reach are
// Prepare targets. Zillow/Realtor.com are NOT upload targets — they ingest an
// MLS feed (Stellar). They stay in the enum/ledger so old rows validate, but
// must never appear as a Prepare checkbox. Single source shared by the UI and
// the server action.
// ---------------------------------------------------------------------------

/** The only channels that may appear as a Prepare checkbox / target. */
export const PREPARE_CHANNELS: readonly SyndicationChannel[] = [
  'culebraluxe',
  'stellar_mls',
  'facebook_marketplace',
  'clasificados',
]

const PREPARE_ALLOWED = new Set<string>(PREPARE_CHANNELS)

export function isPrepareChannel(value: string): value is SyndicationChannel {
  return PREPARE_ALLOWED.has(value)
}

/**
 * Copy shown in the workbench strip. Zillow / Realtor.com / Homes.com are never
 * upload targets here.
 */
export const REACHES_VIA_STELLAR_NOTE =
  'Zillow, Realtor.com, and Homes.com are not upload targets. They update when this listing is live in Stellar and distribution is on. Paste a public URL later to pin it on the constellation.'

/** Blocked / not-this-quarter channels hidden behind a collapsed group. */
export const MORE_CHANNELS: readonly SyndicationChannel[] = [
  'amplia_mls',
  'hubspot',
  'pr_mls',
]
