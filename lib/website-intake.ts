import { randomUUID } from 'node:crypto'

import { normalizeDisplayName, normalizeEmail } from './crm-intake-normalization'
import { prepareInboundEvent } from './crm-intake'
import { resolveOrCreateInboundPerson } from './crm-person-creation'
import type {
  AdaptedWebsiteIntake,
  WebsiteIntakeActorContext,
  WebsiteIntakeDependencies,
  WebsiteIntakePayload,
  WebsiteIntakeRequestType,
  WebsiteIntakeResult,
  ParsedWebsiteIntakeForm,
} from './website-intake-types'

const UUID_PATTERN = /^[0-9a-f]{8}-[0-9a-f]{4}-[1-5][0-9a-f]{3}-[89ab][0-9a-f]{3}-[0-9a-f]{12}$/i
const REQUEST_TYPES = new Set<WebsiteIntakeRequestType>([
  'private_viewing',
  'property_information',
])

export class WebsiteIntakeTransientError extends Error {
  constructor(message: string, options?: ErrorOptions) {
    super(message, options)
    this.name = 'WebsiteIntakeTransientError'
  }
}

async function requireTransition(transition: Promise<boolean>) {
  if (!(await transition)) {
    throw new Error('Website intake receipt transition lost ownership.')
  }
}

function requiredText(value: unknown, field: string) {
  if (typeof value !== 'string') throw new Error(`${field} is required.`)
  const normalized = value.normalize('NFKC').trim().replace(/\s+/g, ' ')
  if (!normalized) throw new Error(`${field} is required.`)
  return normalized
}

export function normalizeWebsiteIntake(input: Record<string, unknown>) {
  const submissionId = requiredText(input.submissionId, 'Submission ID')
  const propertyId = requiredText(input.propertyId, 'Property ID')
  if (!UUID_PATTERN.test(submissionId) || !UUID_PATTERN.test(propertyId)) {
    throw new Error('Submission ID and property ID must be UUIDs.')
  }

  const requestType = requiredText(input.requestType, 'Request type')
  if (!REQUEST_TYPES.has(requestType as WebsiteIntakeRequestType)) {
    throw new Error('Request type is invalid.')
  }

  const displayName = normalizeDisplayName(requiredText(input.name, 'Name'))
  const email = normalizeEmail(requiredText(input.email, 'Email'))
  const message =
    typeof input.message === 'string'
      ? input.message.normalize('NFKC').trim() || undefined
      : undefined

  if (displayName.length > 200) throw new Error('Name is too long.')
  if (email.length > 320) throw new Error('Email is too long.')
  if (message && message.length > 4000) throw new Error('Message is too long.')

  return {
    submissionId,
    requestType: requestType as WebsiteIntakeRequestType,
    propertyId,
    displayName,
    email,
    message,
  } satisfies WebsiteIntakePayload
}

export function parseWebsiteIntakeFormData(
  formData: Pick<FormData, 'get'>,
): ParsedWebsiteIntakeForm {
  if (String(formData.get('company') ?? '').trim()) return { honeypot: true }

  return {
    honeypot: false,
    payload: normalizeWebsiteIntake({
      submissionId: formData.get('submissionId'),
      requestType: formData.get('requestType'),
      propertyId: formData.get('propertyId'),
      name: formData.get('name'),
      email: formData.get('email'),
      message: formData.get('message'),
    }),
  }
}

export function websitePayloadsEqual(
  left: WebsiteIntakePayload,
  right: WebsiteIntakePayload,
) {
  return (
    left.submissionId === right.submissionId &&
    left.requestType === right.requestType &&
    left.propertyId === right.propertyId &&
    left.displayName === right.displayName &&
    left.email === right.email &&
    (left.message ?? undefined) === (right.message ?? undefined)
  )
}

export function adaptWebsiteIntake(
  payload: WebsiteIntakePayload,
  occurredAt: string,
  actor: WebsiteIntakeActorContext = {},
): AdaptedWebsiteIntake {
  return {
    payload,
    event: {
      source: { system: 'website', externalId: payload.submissionId },
      occurredAt,
      channel: 'website',
      eventType:
        payload.requestType === 'private_viewing'
          ? 'private_viewing_requested'
          : 'property_inquiry_submitted',
      direction: 'inbound',
      actor: {
        personId: actor.personId,
        displayNameHint: payload.displayName,
        roleHint: 'buyer',
        identityHints: [
          {
            kind: 'email',
            value: payload.email,
            evidence: actor.emailEvidence ?? 'user_supplied',
          },
        ],
      },
      content: {
        subject:
          payload.requestType === 'private_viewing'
            ? 'Private viewing request'
            : 'Property information request',
        summary: payload.message,
      },
      context: { propertyId: payload.propertyId },
      intentHints: { requestedAction: payload.requestType },
      rawMetadata: { requestType: payload.requestType },
    },
  }
}

const publicAccepted: WebsiteIntakeResult = {
  accepted: true,
  status: 'accepted',
}

export async function processWebsiteIntake(
  payload: WebsiteIntakePayload,
  dependencies: WebsiteIntakeDependencies,
  options: { trustedResolutionRetry?: boolean } = {},
): Promise<WebsiteIntakeResult> {
  const { repositories } = dependencies
  const property = await repositories.findActiveProperty(payload.propertyId)
  if (!property) return { accepted: false, status: 'invalid' }

  const inserted = await repositories.insertOrReadReceipt(payload)
  if (!websitePayloadsEqual(inserted.receipt, payload)) {
    return { accepted: false, status: 'invalid' }
  }

  if (inserted.receipt.status === 'completed') return publicAccepted
  if (
    inserted.receipt.status === 'rejected' ||
    (inserted.receipt.status === 'resolution_required' &&
      !options.trustedResolutionRetry)
  ) {
    return publicAccepted
  }

  const claimed = await repositories.claimReceipt(payload.submissionId, options)
  if (!claimed) return publicAccepted
  if (!claimed.processingStartedAt) {
    throw new Error('Claimed receipt is missing its processing timestamp.')
  }
  const claimToken = claimed.processingStartedAt

  const occurredAt = (dependencies.now ?? (() => new Date()))().toISOString()
  const adapted = adaptWebsiteIntake(
    payload,
    occurredAt,
    dependencies.actorContext,
  )

  try {
    const normalized = (await prepareInboundEvent(adapted.event, repositories.crm))
      .normalizedEvent
    const person = await resolveOrCreateInboundPerson(
      normalized,
      dependencies.personPolicy ?? {
        allowCreation: Boolean(dependencies.actorContext?.allowPersonCreation),
        role: 'buyer',
      },
      repositories.crm,
      dependencies.createId,
    )

    if (person.status === 'resolution_required') {
      await requireTransition(repositories.transitionReceipt({
        submissionId: payload.submissionId,
        claimToken,
        from: 'processing',
        to: 'resolution_required',
      }))
      return publicAccepted
    }

    if (person.status === 'conflicting' || person.status === 'rejected') {
      await requireTransition(repositories.transitionReceipt({
        submissionId: payload.submissionId,
        claimToken,
        from: 'processing',
        to: 'rejected',
      }))
      return publicAccepted
    }

    const duplicateInteractionId = person.existingInteractionId
    if (person.status === 'duplicate' && duplicateInteractionId) {
      await requireTransition(repositories.transitionReceipt({
        submissionId: payload.submissionId,
        claimToken,
        from: 'processing',
        to: 'completed',
        interactionId: duplicateInteractionId,
      }))
      return publicAccepted
    }

    if (!person.personId) throw new Error('Resolved person ID is missing.')

    const canonical = await prepareInboundEvent(
      {
        ...adapted.event,
        actor: { ...adapted.event.actor, personId: person.personId },
      },
      repositories.crm,
    )

    if (canonical.status === 'duplicate' && canonical.existingInteractionId) {
      await requireTransition(repositories.transitionReceipt({
        submissionId: payload.submissionId,
        claimToken,
        from: 'processing',
        to: 'completed',
        interactionId: canonical.existingInteractionId,
      }))
      return publicAccepted
    }
    if (canonical.status !== 'ready') throw new Error('Canonical intake was not ready.')

    const persisted = await repositories.persistCanonical({
      interactionId: (dependencies.createId ?? randomUUID)(),
      personId: person.personId,
      propertyId: property.id,
      submissionId: payload.submissionId,
      requestType: payload.requestType,
      occurredAt,
      displayName: payload.displayName,
      email: payload.email,
      message: payload.message,
    })

    await requireTransition(repositories.transitionReceipt({
      submissionId: payload.submissionId,
      claimToken,
      from: 'processing',
      to: 'completed',
      interactionId: persisted.interactionId,
    }))
    return publicAccepted
  } catch (error) {
    if (error instanceof WebsiteIntakeTransientError) {
      await requireTransition(repositories.transitionReceipt({
        submissionId: payload.submissionId,
        claimToken,
        from: 'processing',
        to: 'received',
      }))
    }
    throw error
  }
}
