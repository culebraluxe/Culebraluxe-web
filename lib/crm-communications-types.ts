import type { InboundEvent, NormalizedIntakeResult } from './crm-intake-types'
import type { PersonCreationResult, PersonRole } from './crm-person-types'

export type CommunicationsTransport = 'call' | 'sms' | 'imessage'
export type CommunicationsDirection = 'inbound' | 'outbound'
export type CommunicationsAssurance =
  | 'transport_observed'
  | 'ownership_verified'
  | 'authenticated_actor'
export type CallDisposition =
  | 'connected'
  | 'no_answer'
  | 'busy'
  | 'failed'
  | 'canceled'
  | 'voicemail'

export type CommunicationsEndpoint =
  | { kind: 'address'; value: string }
  | { kind: 'withheld' }

export interface CommunicationsProviderEvent {
  provider: string
  accountNamespace: string
  transport: CommunicationsTransport
  providerEventId: string
  occurredAt: string | Date
  from: CommunicationsEndpoint[]
  to: CommunicationsEndpoint[]
  trustedDirection?: CommunicationsDirection
  actorAssurance: CommunicationsAssurance
  callDisposition?: CallDisposition
  durationSeconds?: number
  plainText?: string
  correlationId?: string
  displayNameHint?: string
  trustedContext?: InboundEvent['context']
}

export interface OwnedCommunicationsLine {
  phone: string
  creationRole?: PersonRole
}

export interface CommunicationsAdapterConfiguration {
  ownedLines: OwnedCommunicationsLine[]
  sharedExternalPhones?: string[]
  systemEndpoints?: string[]
}

export type AcceptedCommunicationsEvent = {
  status: 'accepted'
  direction: CommunicationsDirection
  actorPhone: string
  applicableCreationRole?: PersonRole
  inboundEvent: InboundEvent
}

export type CommunicationsAdapterResult =
  | AcceptedCommunicationsEvent
  | { status: 'excluded'; reason: string }
  | { status: 'resolution_required'; reason: string }
  | { status: 'rejected'; reason: string }

export type CommunicationsIntakeResult =
  | { status: 'excluded' | 'rejected'; reason: string }
  | {
      status: 'resolution_required'
      reason: string
      personResult?: PersonCreationResult
    }
  | { status: 'duplicate'; existingInteractionId: string }
  | {
      status: 'ready'
      personResult: PersonCreationResult
      intakeResult: NormalizedIntakeResult
    }
