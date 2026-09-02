export const SYNDICATION_CHANNELS = [
  'culebraluxe',
  'clasificados',
  'facebook_marketplace',
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
  clasificados: {
    id: 'clasificados',
    label: 'ClasificadosOnline',
    shortLabel: 'Clasificados',
    mode: 'copy_pack',
    readiness: 'pack',
    defaultTtlDays: 40,
    notes: 'No publisher API. Generate pack, paste once, confirm the live URL.',
  },
  facebook_marketplace: {
    id: 'facebook_marketplace',
    label: 'Facebook Marketplace',
    shortLabel: 'Marketplace',
    mode: 'copy_pack',
    readiness: 'pack',
    defaultTtlDays: 30,
    notes: 'Page listing for now. Graph publish later — Business certified + Tech Partner.',
  },
  pr_mls: {
    id: 'pr_mls',
    label: 'PR Realtors MLS',
    shortLabel: 'PR MLS',
    mode: 'mls',
    readiness: 'blocked',
    defaultTtlDays: null,
    notes: 'Requires PRAR / board membership. License alone is not enough.',
  },
  amplia_mls: {
    id: 'amplia_mls',
    label: 'Amplia MLS',
    shortLabel: 'Amplia',
    mode: 'mls',
    readiness: 'stub',
    defaultTtlDays: null,
    notes: 'Possible PR-license path to portals. Confirm syndication in writing.',
  },
  zillow_fsbo: {
    id: 'zillow_fsbo',
    label: 'Zillow FSBO',
    shortLabel: 'Zillow',
    mode: 'copy_pack',
    readiness: 'pack',
    defaultTtlDays: null,
    notes: 'Manual owner card only. Full Zillow search needs an MLS feed.',
  },
  realtor_com: {
    id: 'realtor_com',
    label: 'Realtor.com',
    shortLabel: 'Realtor.com',
    mode: 'blocked',
    readiness: 'blocked',
    defaultTtlDays: null,
    notes: 'MLS-only. No direct broker upload.',
  },
  hubspot: {
    id: 'hubspot',
    label: 'HubSpot',
    shortLabel: 'HubSpot',
    mode: 'api',
    readiness: 'stub',
    defaultTtlDays: null,
    notes: 'Sibling CRM. Account exists; this ledger does not write into HubSpot yet.',
  },
}

export function isSyndicationChannel(value: string): value is SyndicationChannel {
  return (SYNDICATION_CHANNELS as readonly string[]).includes(value)
}
