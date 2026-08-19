import type { InboundEvent, NormalizedIntakeResult } from './crm-intake-types'
import type { PersonCreationResult, PersonRole } from './crm-person-types'

export type EmailDirection = 'inbound' | 'outbound'
export type EmailSenderAuthentication = 'unverified' | 'authenticated_pass'
export type EmailMessageCategory =
  | 'human_correspondence'
  | 'system_notification'
  | 'delivery_status'
  | 'auto_reply'
  | 'bulk_list'

export interface EmailMailbox {
  email: string
  displayName?: string
}

export interface EmailAttachmentDescriptor {
  providerAttachmentId: string
  filename: string
  mimeType: string
  sizeBytes: number
}

export interface EmailTransportEvidence {
  autoSubmitted?: string
  listId?: string
  contentType?: string
  returnPath?: string
  deliveryStatus?: boolean
}

export interface EmailProviderMessage {
  provider: string
  accountNamespace: string
  messageId: string
  threadId?: string
  inReplyToMessageId?: string
  referenceMessageIds?: string[]
  occurredAt: string | Date
  senders: EmailMailbox[]
  to?: EmailMailbox[]
  cc?: EmailMailbox[]
  bcc?: EmailMailbox[]
  replyTo?: EmailMailbox[]
  trustedDirection?: EmailDirection
  senderAuthentication: EmailSenderAuthentication
  category: EmailMessageCategory
  subject?: string
  plainText?: string
  isForward?: boolean
  attachments?: EmailAttachmentDescriptor[]
  transportEvidence?: EmailTransportEvidence
  trustedContext?: InboundEvent['context']
}

export interface InternalEmailMailbox {
  email: string
  creationRole?: PersonRole
}

export interface EmailAdapterConfiguration {
  internalMailboxes: InternalEmailMailbox[]
  systemSenderEmails?: string[]
  noReplySenderEmails?: string[]
}

export type AcceptedEmail = {
  status: 'accepted'
  direction: EmailDirection
  actorEmail: string
  applicableCreationRole?: PersonRole
  inboundEvent: InboundEvent
}

export type EmailAdapterResult =
  | AcceptedEmail
  | { status: 'excluded'; reason: string }
  | { status: 'resolution_required'; reason: string }
  | { status: 'rejected'; reason: string }

export type EmailIntakeResult =
  | { status: 'excluded' | 'rejected'; reason: string }
  | { status: 'resolution_required'; reason: string; personResult?: PersonCreationResult }
  | { status: 'duplicate'; existingInteractionId: string }
  | {
      status: 'ready'
      personResult: PersonCreationResult
      intakeResult: NormalizedIntakeResult
    }

