import { normalizeEmail, sanitizeRawMetadata } from './crm-intake-normalization'
import type { JsonObject, JsonValue } from './crm-types'
import type {
  AcceptedEmail,
  EmailAdapterConfiguration,
  EmailAdapterResult,
  EmailAttachmentDescriptor,
  EmailDirection,
  EmailMailbox,
  EmailMessageCategory,
  EmailProviderMessage,
  EmailSenderAuthentication,
} from './crm-email-types'

const SOURCE_TOKEN = /^[a-z0-9](?:[a-z0-9_-]{0,62}[a-z0-9])?$/
const ATTACHMENT_ID = /^[A-Za-z0-9._~+=-]{1,512}$/
const CONTROL_CHARACTERS = /[\u0000-\u001f\u007f-\u009f]/
const URL_LIKE = /^[a-z][a-z0-9+.-]*:\/\//i
const MAX_OPAQUE_ID_LENGTH = 512
const MAX_REFERENCE_IDS = 100

const CATEGORIES = new Set<EmailMessageCategory>([
  'human_correspondence',
  'system_notification',
  'delivery_status',
  'auto_reply',
  'bulk_list',
])
const AUTHENTICATION = new Set<EmailSenderAuthentication>([
  'unverified',
  'authenticated_pass',
])
const CREATION_ROLES = new Set(['buyer', 'seller', 'both'])

function normalizeSourceToken(value: string, field: string) {
  if (typeof value !== 'string') {
    throw new Error(`${field} must be a string.`)
  }
  const normalized = value.normalize('NFKC').trim().toLowerCase()
  if (!SOURCE_TOKEN.test(normalized)) {
    throw new Error(`${field} must be a valid 1-64 character source token.`)
  }
  return normalized
}

function opaqueId(value: string, field: string) {
  if (
    typeof value !== 'string' ||
    value.length < 1 ||
    value.length > MAX_OPAQUE_ID_LENGTH ||
    !/\S/.test(value) ||
    CONTROL_CHARACTERS.test(value)
  ) {
    throw new Error(`${field} must be a bounded opaque identifier.`)
  }
  return value
}

function boundedText(value: string | undefined, maximum: number, field: string) {
  if (value === undefined) return undefined
  if (typeof value !== 'string') throw new Error(`${field} must be a string.`)
  const normalized = value.normalize('NFKC').trim()
  if (normalized.length > maximum) {
    throw new Error(`${field} exceeds ${maximum} characters.`)
  }
  return normalized || undefined
}

function stableUnique(values: string[]) {
  const seen = new Set<string>()
  return values.filter((value) => {
    if (seen.has(value)) return false
    seen.add(value)
    return true
  })
}

function normalizedMailboxes(mailboxes: EmailMailbox[] | undefined) {
  return stableUnique((mailboxes ?? []).map(({ email }) => normalizeEmail(email)))
}

function normalizeAttachments(attachments: EmailAttachmentDescriptor[] = []) {
  return attachments.map((attachment) => {
    if (
      !attachment ||
      typeof attachment !== 'object' ||
      typeof attachment.providerAttachmentId !== 'string' ||
      !ATTACHMENT_ID.test(attachment.providerAttachmentId) ||
      URL_LIKE.test(attachment.providerAttachmentId)
    ) {
      throw new Error('Attachment provider ID must be an opaque non-URL identifier.')
    }
    if (!Number.isSafeInteger(attachment.sizeBytes) || attachment.sizeBytes < 0) {
      throw new Error('Attachment sizeBytes must be a non-negative safe integer.')
    }
    if (
      typeof attachment.filename !== 'string' ||
      typeof attachment.mimeType !== 'string'
    ) {
      throw new Error('Attachment filename and MIME type must be strings.')
    }
    const filename = boundedText(attachment.filename, 512, 'Attachment filename')
    const mimeType = boundedText(attachment.mimeType, 255, 'Attachment MIME type')
    if (!filename || !mimeType) {
      throw new Error('Attachment filename and MIME type are required.')
    }
    return {
      providerAttachmentId: attachment.providerAttachmentId,
      filename,
      mimeType,
      sizeBytes: attachment.sizeBytes,
    }
  })
}

function classifyDirection(
  sender: string,
  recipients: string[],
  internal: Set<string>,
): EmailDirection | 'internal' | 'ambiguous' {
  const senderInternal = internal.has(sender)
  const externalRecipients = stableUnique(
    recipients.filter((recipient) => !internal.has(recipient)),
  )

  if (senderInternal) {
    if (externalRecipients.length === 0) return 'internal'
    if (externalRecipients.length > 1) return 'ambiguous'
    return 'outbound'
  }

  if (recipients.length > 0 && externalRecipients.length === 0) return 'inbound'
  return 'ambiguous'
}

function applicableRole(
  direction: EmailDirection,
  sender: string,
  recipients: string[],
  configured: Map<string, EmailAdapterConfiguration['internalMailboxes'][number]>,
) {
  const addresses =
    direction === 'inbound'
      ? stableUnique(recipients.filter((email) => configured.has(email)))
      : [sender]
  const roles = addresses.map((email) => configured.get(email)?.creationRole)
  if (addresses.length === 0 || roles.some((role) => !role)) return undefined
  const unique = new Set(roles)
  return unique.size === 1 ? roles[0] : undefined
}

function exactTransportExclusion(
  message: EmailProviderMessage,
  sender: string,
  systemSenders: Set<string>,
  noReplySenders: Set<string>,
) {
  if (message.category !== 'human_correspondence') return message.category
  if (systemSenders.has(sender)) return 'configured_system_sender'
  if (noReplySenders.has(sender)) return 'configured_no_reply_sender'

  const evidence = message.transportEvidence
  if (!evidence) return undefined
  if (evidence.deliveryStatus) return 'delivery_status'
  const normalizedMimeType = evidence.contentType
    ?.split(';', 1)[0]
    .trim()
    .toLowerCase()
  if (normalizedMimeType === 'multipart/report') {
    return 'delivery_status'
  }
  if (evidence.returnPath?.trim() === '<>') return 'null_return_path'
  if (evidence.autoSubmitted?.trim().toLowerCase() !== undefined &&
      evidence.autoSubmitted.trim().toLowerCase() !== 'no') {
    return 'auto_submitted'
  }
  if (evidence.listId?.trim()) return 'list_mail'
  return undefined
}

export function adaptEmailMessage(
  message: EmailProviderMessage,
  configuration: EmailAdapterConfiguration,
): EmailAdapterResult {
  try {
    const provider = normalizeSourceToken(message.provider, 'provider')
    const accountNamespace = normalizeSourceToken(
      message.accountNamespace,
      'accountNamespace',
    )
    const messageId = opaqueId(message.messageId, 'messageId')
    if (!CATEGORIES.has(message.category)) throw new Error('Email category is invalid.')
    if (!AUTHENTICATION.has(message.senderAuthentication)) {
      throw new Error('Sender authentication verdict is invalid.')
    }
    if (message.senders.length !== 1) {
      throw new Error('Exactly one sender mailbox is required.')
    }

    const sender = normalizeEmail(message.senders[0].email)
    const toEmails = normalizedMailboxes(message.to)
    const ccEmails = normalizedMailboxes(message.cc)
    const bccEmails = normalizedMailboxes(message.bcc)
    const replyToEmails = normalizedMailboxes(message.replyTo)
    const recipients = stableUnique([...toEmails, ...ccEmails, ...bccEmails])

    const configured = new Map<
      string,
      EmailAdapterConfiguration['internalMailboxes'][number]
    >()
    for (const mailbox of configuration.internalMailboxes) {
      const email = normalizeEmail(mailbox.email)
      if (mailbox.creationRole && !CREATION_ROLES.has(mailbox.creationRole)) {
        throw new Error('Internal mailbox creation role is invalid.')
      }
      const prior = configured.get(email)
      if (prior && prior.creationRole !== mailbox.creationRole) {
        throw new Error('Internal mailbox configuration has conflicting roles.')
      }
      configured.set(email, { ...mailbox, email })
    }
    const internal = new Set(configured.keys())
    const systemSenders = new Set((configuration.systemSenderEmails ?? []).map(normalizeEmail))
    const noReplySenders = new Set((configuration.noReplySenderEmails ?? []).map(normalizeEmail))

    const exclusion = exactTransportExclusion(
      message,
      sender,
      systemSenders,
      noReplySenders,
    )
    if (exclusion) return { status: 'excluded', reason: exclusion }

    const derivedDirection = classifyDirection(sender, recipients, internal)
    if (derivedDirection === 'internal') {
      return { status: 'excluded', reason: 'internal_only' }
    }
    if (derivedDirection === 'ambiguous') {
      const senderInternal = internal.has(sender)
      const externalRecipients = recipients.filter((email) => !internal.has(email))
      if (senderInternal && new Set(externalRecipients).size > 1) {
        return { status: 'resolution_required', reason: 'multiple_external_recipients' }
      }
      return { status: 'rejected', reason: 'ambiguous_envelope' }
    }
    if (
      message.trustedDirection &&
      message.trustedDirection !== derivedDirection
    ) {
      return { status: 'rejected', reason: 'conflicting_trusted_direction' }
    }

    const actorEmail =
      derivedDirection === 'inbound'
        ? sender
        : recipients.find((email) => !internal.has(email))!
    if ((message.referenceMessageIds?.length ?? 0) > MAX_REFERENCE_IDS) {
      throw new Error('referenceMessageIds exceeds 100 entries.')
    }
    const referenceMessageIds = stableUnique(
      (message.referenceMessageIds ?? []).map((id, index) =>
        opaqueId(id, `referenceMessageIds[${index}]`),
      ),
    )
    const threadId = message.threadId !== undefined
      ? opaqueId(message.threadId, 'threadId')
      : undefined
    const inReplyToMessageId = message.inReplyToMessageId !== undefined
      ? opaqueId(message.inReplyToMessageId, 'inReplyToMessageId')
      : undefined
    const attachments = normalizeAttachments(message.attachments)
    const occurredAt = new Date(message.occurredAt)
    if (Number.isNaN(occurredAt.getTime())) throw new Error('occurredAt is invalid.')

    const metadata: JsonObject = {}
    if (threadId) metadata.threadId = threadId
    if (inReplyToMessageId) metadata.inReplyToMessageId = inReplyToMessageId
    if (referenceMessageIds.length) metadata.referenceMessageIds = referenceMessageIds
    if (message.isForward !== undefined) metadata.isForward = message.isForward
    if (toEmails.length) metadata.toEmails = toEmails
    if (ccEmails.length) metadata.ccEmails = ccEmails
    if (replyToEmails.length) metadata.replyToEmails = replyToEmails
    if (attachments.length) metadata.attachments = attachments as unknown as JsonValue

    const direction = derivedDirection
    const result: AcceptedEmail = {
      status: 'accepted',
      direction,
      actorEmail,
      applicableCreationRole: applicableRole(direction, sender, recipients, configured),
      inboundEvent: {
        source: {
          system: `email:${provider}:${accountNamespace}`,
          externalId: messageId,
        },
        occurredAt: occurredAt.toISOString(),
        channel: 'email',
        eventType: direction === 'inbound' ? 'email_received' : 'email_sent',
        direction,
        actor: {
          identityHints: [
            {
              kind: 'email',
              value: actorEmail,
              evidence:
                direction === 'inbound' &&
                message.senderAuthentication === 'authenticated_pass'
                  ? 'provider_asserted'
                  : 'user_supplied',
            },
          ],
          displayNameHint:
            direction === 'inbound'
              ? boundedText(message.senders[0].displayName, 200, 'Sender display name')
              : undefined,
          roleHint: applicableRole(direction, sender, recipients, configured),
        },
        content: {
          subject: boundedText(message.subject, 500, 'subject'),
          summary: boundedText(message.plainText, 4000, 'plainText'),
        },
        context: message.trustedContext,
        rawMetadata: sanitizeRawMetadata(metadata),
      },
    }
    return result
  } catch (error) {
    return {
      status: 'rejected',
      reason: error instanceof Error ? error.message : 'Invalid email message.',
    }
  }
}
