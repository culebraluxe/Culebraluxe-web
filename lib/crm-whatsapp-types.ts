import type { InboundEvent, NormalizedIntakeResult } from './crm-intake-types'
import type { PersonCreationResult, PersonRole } from './crm-person-types'

export type WhatsAppDirection = 'inbound' | 'outbound'
export type WhatsAppAssurance = 'transport_observed'
export type WhatsAppMessageClass =
  | 'free_form'
  | 'template'
  | 'service'
  | 'system'

export type WhatsAppEndpoint =
  | { kind: 'address'; value: string }
  | { kind: 'withheld' }

/**
 * Provider-neutral WhatsApp message facts, ready for a future connector to
 * lower a provider webhook into. Only normalized transport facts cross into
 * CRM; provider SDK objects, signature headers, access tokens, and phone
 * number IDs never do.
 *
 * A signed webhook proves delivery integrity of the owned business line, not
 * ownership of the external sender/recipient, so actor assurance is limited
 * to `transport_observed` and never enables person auto-creation.
 *
 * CRM-07 readiness scaffolding: produces canonical in-memory inputs only.
 * No interaction, task, interest, or person write is reachable and no
 * provider is contacted. WhatsApp identity resolves through canonical phone
 * (strict E.164); WhatsApp is a channel, not a new identity type.
 */
export interface WhatsAppProviderEvent {
  provider: string
  accountNamespace: string
  providerMessageId: string
  occurredAt: string | Date
  from: WhatsAppEndpoint[]
  to: WhatsAppEndpoint[]
  trustedDirection?: WhatsAppDirection
  actorAssurance: WhatsAppAssurance
  contentClass: WhatsAppMessageClass
  plainText?: string
  templateId?: string
  correlationId?: string
  displayNameHint?: string
  trustedContext?: InboundEvent['context']
}

export interface OwnedWhatsAppLine {
  phone: string
  creationRole?: PersonRole
}

export interface WhatsAppAdapterConfiguration {
  ownedLines: OwnedWhatsAppLine[]
  sharedExternalPhones?: string[]
  systemEndpoints?: string[]
}

export type AcceptedWhatsAppEvent = {
  status: 'accepted'
  direction: WhatsAppDirection
  actorPhone: string
  applicableCreationRole?: PersonRole
  inboundEvent: InboundEvent
}

export type WhatsAppAdapterResult =
  | AcceptedWhatsAppEvent
  | { status: 'excluded'; reason: string }
  | { status: 'resolution_required'; reason: string }
  | { status: 'rejected'; reason: string }

export type WhatsAppIntakeResult =
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
