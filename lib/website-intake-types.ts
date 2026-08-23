import type { InboundEvent, IntakeRepositories } from './crm-intake-types'
import type {
  PersonCreationPolicy,
  PersonCreationRepositories,
} from './crm-person-types'

export type WebsiteIntakeRequestType =
  | 'private_viewing'
  | 'property_information'
  | 'general_enquiry'

export type WebsiteIntakeStatus =
  | 'received'
  | 'processing'
  | 'resolution_required'
  | 'completed'
  | 'rejected'

export interface WebsiteIntakePayload {
  submissionId: string
  requestType: WebsiteIntakeRequestType
  propertyId?: string
  displayName: string
  email: string
  message?: string
  // Service intent carried from a service-specific CTA on the general-enquiry
  // path. Allow-listed against the supported services vocabulary; stored on the
  // canonical interaction's source_metadata, never on the receipt.
  service?: string
}

export interface WebsiteIntakeReceipt extends WebsiteIntakePayload {
  status: WebsiteIntakeStatus
  processingStartedAt?: string
  interactionId?: string
  createdAt: string
  updatedAt: string
}

export interface WebsiteIntakeActorContext {
  personId?: string
  emailEvidence?: 'authenticated' | 'provider_asserted'
  allowPersonCreation?: boolean
}

export type WebsiteIntakeResult = {
  accepted: boolean
  status: 'accepted' | 'invalid' | 'unavailable'
  fieldErrors?: Partial<Record<'name' | 'email' | 'message', string>>
}

export interface WebsiteIntakeRepositories {
  findActiveProperty(propertyId: string): Promise<{ id: string } | null>
  insertOrReadReceipt(
    payload: WebsiteIntakePayload,
  ): Promise<{ receipt: WebsiteIntakeReceipt; created: boolean }>
  claimReceipt(
    submissionId: string,
    options?: { trustedResolutionRetry?: boolean },
  ): Promise<WebsiteIntakeReceipt | null>
  transitionReceipt(input: {
    submissionId: string
    claimToken: string
    from: WebsiteIntakeStatus
    to: Exclude<WebsiteIntakeStatus, 'processing'>
    interactionId?: string
  }): Promise<boolean>
  crm: IntakeRepositories & PersonCreationRepositories
  persistCanonical(input: CanonicalWebsiteIntakeInput): Promise<{
    interactionId: string
    created: boolean
  }>
}

export interface CanonicalWebsiteIntakeInput {
  interactionId: string
  personId: string
  propertyId?: string
  submissionId: string
  requestType: WebsiteIntakeRequestType
  occurredAt: string
  displayName: string
  email: string
  message?: string
  service?: string
}

export interface WebsiteIntakeDependencies {
  repositories: WebsiteIntakeRepositories
  personPolicy?: PersonCreationPolicy
  actorContext?: WebsiteIntakeActorContext
  createId?: () => string
  now?: () => Date
}

export interface AdaptedWebsiteIntake {
  payload: WebsiteIntakePayload
  event: InboundEvent
}

export type ParsedWebsiteIntakeForm =
  | { honeypot: true }
  | { honeypot: false; payload: WebsiteIntakePayload }
