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

export type InteractionChannel =
  | "email"
  | "call"
  | "imessage"
  | "sms"
  | "meeting"
  | "showing"
  | "note"

export type InteractionDirection =
  | "inbound"
  | "outbound"

export type DealStage =
  | "new_lead"
  | "qualified"
  | "showing"
  | "offer"
  | "under_contract"
  | "closed"

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
  direction?: InteractionDirection
  occurredAt: string
  title: string
  summary?: string
  durationSeconds?: number
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
}