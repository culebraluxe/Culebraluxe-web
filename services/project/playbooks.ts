import type { WbsCategoryId } from '../wbs'

export type PlaybookNodeDefinition = {
  key: string
  title: string
  category: WbsCategoryId
  parentKey?: string
  order: number
}

export type PlaybookDefinition = {
  id: string
  version: number
  projectType: string
  nodes: readonly PlaybookNodeDefinition[]
}

/** Canonical first playbook: every listing starts with the same operational spine. */
export const LISTING_ONBOARDING_V1: PlaybookDefinition = {
  id: 'listing-onboarding', version: 1, projectType: 'listing',
  nodes: [
    { key: 'parties', title: 'Clients / Parties', category: 'clients', order: 1 },
    { key: 'property', title: 'Property', category: 'properties', order: 2 },
    { key: 'agreement', title: 'Listing Agreement', category: 'contracts', order: 3 },
    { key: 'signature', title: 'Seller Signature', category: 'contracts', parentKey: 'agreement', order: 1 },
    { key: 'media', title: 'Cabinet + Photos', category: 'media', order: 4 },
    { key: 'marketing', title: 'Coming Soon / Marketing', category: 'marketing', order: 5 },
    { key: 'accounting', title: 'Commission / Accounting', category: 'accounting', order: 6 },
  ],
}

export const PROJECT_PLAYBOOKS: readonly PlaybookDefinition[] = [LISTING_ONBOARDING_V1]

export function getPlaybook(id: string, version: number): PlaybookDefinition | null {
  return PROJECT_PLAYBOOKS.find(playbook => playbook.id === id && playbook.version === version) ?? null
}
