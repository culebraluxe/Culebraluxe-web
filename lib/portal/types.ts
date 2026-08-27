import type {
  InteractionChannel,
  InteractionDirection,
  JsonObject,
} from '@/lib/crm-types'

export type {
  InteractionChannel,
  InteractionDirection,
} from '@/lib/crm-types'

export type ClientRole = "buyer" | "seller" | "both"

export type ClientStatus =
  | "new"
  | "warm"
  | "active"
  | "referral"

export type PropertyInterestStatus =
  | "interested"
  | "shortlisted"
  | "tour_completed"

export type DealStage =
  | "new_lead"
  | "qualified"
  | "showing"
  | "offer"
  | "under_contract"
  | "closed"

/** Canonical financing applicability fact (NULL = unknown). */
export type DealFinancingType = "cash" | "financed"

export interface PropertyInterest {
  id: string
  propertyId: string
  propertyName: string
  location: string
  price: number
  bedrooms?: number
  descriptor?: string
  status: PropertyInterestStatus
  heroMediaId?: string
}

export interface Interaction {
  id: string
  channel: InteractionChannel
  eventType: string
  direction?: InteractionDirection
  occurredAt: string
  title: string
  summary?: string
  durationSeconds?: number
  sourceMetadata: JsonObject
}

export interface RelationshipChannelProjection {
  source: string
  channel: string
  observedCommunicationCount: number
  inboundCount: number
  outboundCount: number
  lastObservedAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  twoWay: boolean
  coverageLimited: boolean
}

export interface RelationshipActivity {
  sources: string[]
  inboundCount: number
  outboundCount: number
  observedCommunicationCount: number
  twoWay: boolean
  lastObservedAt: string | null
  lastMeaningfulContactAt: string | null
  lastInboundAt: string | null
  lastOutboundAt: string | null
  coverageLimited: boolean
  /** Source-specific relationship-memory projection (truthful per channel). */
  channels: RelationshipChannelProjection[]
}

export interface Client {
  id: string
  displayName: string
  role: ClientRole
  status: ClientStatus

  location?: string
  email?: string
  phone?: string

  budgetMin?: number
  budgetMax?: number

  preferredAreas?: string[]
  propertyTypes?: string[]
  priorities?: string[]
  timeline?: string

  assignedAgent?: string
  assignedUserId?: string

  lastContact?: {
    channel: InteractionChannel
    occurredAt: string
    summary?: string
  }

  nextAction?: {
    title: string
    occurredAt: string
    detail?: string
  }

  notes?: string

  propertyInterests: PropertyInterest[]
  interactions: Interaction[]
  relationshipActivity?: RelationshipActivity
}

export interface Deal {
  id: string

  propertyId: string
  propertyName: string
  propertyLocation: string

  heroMediaId?: string

  clientId: string
  clientName: string

  stage: DealStage

  listPrice?: number
  offerPrice?: number

  nextMilestone?: string
  nextMilestoneAt?: string

  lastActivity?: string
  lastActivityAt?: string

  owner: string

  closingDate?: string

  propertyDescriptor?: string

  showingCount?: number
  offerCount?: number
  participantCount?: number
  latestOfferAmount?: number
  latestOfferStatus?: string
}
