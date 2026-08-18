import type {
  CreateInteractionInput,
  Interaction,
  InteractionChannel,
  InteractionDirection,
  JsonObject,
} from './crm-types'

export type IdentityEvidence =
  | 'authenticated'
  | 'provider_asserted'
  | 'user_supplied'

export type IdentityHint =
  | {
      kind: 'email'
      value: string
      evidence: IdentityEvidence
    }
  | {
      kind: 'phone'
      value: string
      evidence: IdentityEvidence
    }
  | {
      kind: 'external'
      value: string
      sourceSystem: string
      evidence: IdentityEvidence
    }

export interface InboundEvent {
  source: {
    system: string
    externalId: string
  }
  occurredAt: string | Date
  channel: InteractionChannel
  eventType: string
  direction?: InteractionDirection
  actor: {
    personId?: string
    identityHints: IdentityHint[]
    displayNameHint?: string
    roleHint?: 'buyer' | 'seller' | 'both'
  }
  content?: {
    subject?: string
    summary?: string
    durationSeconds?: number
  }
  context?: {
    propertyId?: string
    propertySlug?: string
    propertyUrl?: string
    dealId?: string
  }
  intentHints?: {
    requestedAction?: RequestedAction
  }
  rawMetadata: JsonObject
}

export type RequestedAction =
  | 'private_viewing'
  | 'property_information'
  | 'seller_consultation'
  | 'saved_property'
  | 'saved_search'

export type NormalizedIdentityHint = IdentityHint & {
  normalizedValue: string
}

export interface NormalizedInboundEvent
  extends Omit<InboundEvent, 'occurredAt' | 'actor' | 'rawMetadata'> {
  occurredAt: string
  actor: Omit<InboundEvent['actor'], 'identityHints'> & {
    identityHints: NormalizedIdentityHint[]
  }
  rawMetadata: JsonObject
}

export interface IdentityMatch {
  identityId: string
  personId: string
  kind: NormalizedIdentityHint['kind']
  normalizedValue: string
}

export interface PersonResolution {
  status: 'resolved' | 'unresolved' | 'ambiguous' | 'conflicting'
  personId?: string
  matchedIdentityIds: string[]
  evidence: Array<{
    kind: IdentityHint['kind']
    normalizedValue: string
    result: 'matched' | 'unmatched' | 'conflict'
  }>
}

export interface ResolvedProperty {
  id: string
  slug?: string
}

export interface ResolvedDeal {
  id: string
  personId: string
  propertyId: string
}

export interface PropertyResolution {
  status: 'not_provided' | 'resolved' | 'unresolved' | 'conflicting'
  property?: ResolvedProperty
}

export interface DealResolution {
  status: 'not_provided' | 'resolved' | 'unresolved' | 'conflicting'
  deal?: ResolvedDeal
}

export interface FollowUpIntent {
  kind: 'human_follow_up'
  reason: 'private_viewing' | 'seller_consultation'
  personId: string
  propertyId?: string
}

export interface PropertyInterestIntent {
  personId: string
  propertyId: string
  requestedStatus: 'interested'
  reason: 'private_viewing' | 'property_information' | 'saved_property'
}

export interface IntakeRepositories {
  findInteractionBySourceIdentity(
    sourceSystem: string,
    sourceExternalId: string,
  ): Promise<Interaction | null>
  personExists(personId: string): Promise<boolean>
  findIdentityMatch(
    hint: NormalizedIdentityHint,
  ): Promise<IdentityMatch | null>
  findPropertyById(propertyId: string): Promise<ResolvedProperty | null>
  findPropertyBySlug(slug: string): Promise<ResolvedProperty | null>
  findDealById(dealId: string): Promise<ResolvedDeal | null>
}

export interface NormalizedIntakeResult {
  status: 'ready' | 'duplicate' | 'resolution_required' | 'rejected'
  normalizedEvent: NormalizedInboundEvent
  interactionInput?: CreateInteractionInput
  existingInteractionId?: string
  personResolution: PersonResolution
  propertyResolution: PropertyResolution
  dealResolution: DealResolution
  propertyInterestIntent?: PropertyInterestIntent
  followUpIntent?: FollowUpIntent
  warnings: string[]
}
