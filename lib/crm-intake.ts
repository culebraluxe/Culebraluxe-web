import type { CreateInteractionInput } from './crm-types'
import type {
  DealResolution,
  FollowUpIntent,
  InboundEvent,
  IntakeRepositories,
  NormalizedInboundEvent,
  NormalizedIntakeResult,
  PropertyInterestIntent,
  PropertyResolution,
  ResolvedProperty,
} from './crm-intake-types'
import {
  extractRecognizedPropertySlug,
  normalizeInboundEvent,
} from './crm-intake-normalization'
import { resolvePerson } from './crm-person-resolution'

const noProperty: PropertyResolution = { status: 'not_provided' }
const noDeal: DealResolution = { status: 'not_provided' }

async function resolveProperty(
  event: NormalizedInboundEvent,
  repositories: IntakeRepositories,
): Promise<PropertyResolution> {
  const context = event.context
  if (
    !context?.propertyId &&
    !context?.propertySlug &&
    !context?.propertyUrl
  ) {
    return noProperty
  }

  const candidates: Array<ResolvedProperty | null> = []

  if (context.propertyId) {
    candidates.push(await repositories.findPropertyById(context.propertyId))
  }
  if (context.propertySlug) {
    candidates.push(
      await repositories.findPropertyBySlug(context.propertySlug),
    )
  }
  if (context.propertyUrl) {
    const slug = extractRecognizedPropertySlug(context.propertyUrl)
    candidates.push(slug ? await repositories.findPropertyBySlug(slug) : null)
  }

  if (candidates.some((candidate) => !candidate)) {
    return { status: 'unresolved' }
  }

  const properties = candidates.filter(
    (candidate): candidate is ResolvedProperty => Boolean(candidate),
  )
  const ids = new Set(properties.map((property) => property.id))

  return ids.size === 1
    ? { status: 'resolved', property: properties[0] }
    : { status: 'conflicting' }
}

async function resolveDeal(
  event: NormalizedInboundEvent,
  repositories: IntakeRepositories,
): Promise<DealResolution> {
  if (!event.context?.dealId) return noDeal
  const deal = await repositories.findDealById(event.context.dealId)
  return deal ? { status: 'resolved', deal } : { status: 'unresolved' }
}

function interactionInput(
  event: NormalizedInboundEvent,
  personId: string,
  propertyId?: string,
  dealId?: string,
): CreateInteractionInput {
  return {
    personId,
    propertyId,
    dealId,
    channel: event.channel,
    eventType: event.eventType,
    direction: event.direction,
    occurredAt: event.occurredAt,
    title: event.content?.subject,
    summary: event.content?.summary,
    durationSeconds: event.content?.durationSeconds,
    sourceSystem: event.source.system,
    sourceExternalId: event.source.externalId,
    sourceMetadata: event.rawMetadata,
  }
}

function deriveIntents(
  event: NormalizedInboundEvent,
  personId: string,
  propertyId?: string,
) {
  const action = event.intentHints?.requestedAction
  let followUpIntent: FollowUpIntent | undefined
  let propertyInterestIntent: PropertyInterestIntent | undefined

  if (action === 'private_viewing' || action === 'seller_consultation') {
    followUpIntent = {
      kind: 'human_follow_up',
      reason: action,
      personId,
      propertyId,
    }
  }

  if (
    propertyId &&
    (action === 'private_viewing' ||
      action === 'property_information' ||
      action === 'saved_property')
  ) {
    propertyInterestIntent = {
      personId,
      propertyId,
      requestedStatus: 'interested',
      reason: action,
    }
  }

  return { followUpIntent, propertyInterestIntent }
}

export async function prepareInboundEvent(
  inboundEvent: InboundEvent,
  repositories: IntakeRepositories,
): Promise<NormalizedIntakeResult> {
  const event = normalizeInboundEvent(inboundEvent)
  const duplicate = await repositories.findInteractionBySourceIdentity(
    event.source.system,
    event.source.externalId,
  )

  if (duplicate) {
    return {
      status: 'duplicate',
      normalizedEvent: event,
      existingInteractionId: duplicate.id,
      personResolution: {
        status: 'resolved',
        personId: duplicate.personId,
        matchedIdentityIds: [],
        evidence: [],
      },
      propertyResolution: duplicate.propertyId
        ? {
            status: 'resolved',
            property: { id: duplicate.propertyId },
          }
        : noProperty,
      dealResolution: noDeal,
      warnings: [],
    }
  }

  const personResolution = await resolvePerson(event, repositories)
  if (personResolution.status !== 'resolved' || !personResolution.personId) {
    return {
      status:
        personResolution.status === 'unresolved'
          ? 'resolution_required'
          : 'rejected',
      normalizedEvent: event,
      personResolution,
      propertyResolution: noProperty,
      dealResolution: noDeal,
      warnings: [],
    }
  }

  const [propertyResolution, dealResolution] = await Promise.all([
    resolveProperty(event, repositories),
    resolveDeal(event, repositories),
  ])

  if (
    propertyResolution.status === 'unresolved' ||
    propertyResolution.status === 'conflicting' ||
    dealResolution.status === 'unresolved'
  ) {
    return {
      status: 'rejected',
      normalizedEvent: event,
      personResolution,
      propertyResolution,
      dealResolution,
      warnings: [],
    }
  }

  const resolvedPropertyId = propertyResolution.property?.id
  const deal = dealResolution.deal
  if (
    deal &&
    (deal.personId !== personResolution.personId ||
      (resolvedPropertyId && deal.propertyId !== resolvedPropertyId))
  ) {
    return {
      status: 'rejected',
      normalizedEvent: event,
      personResolution,
      propertyResolution,
      dealResolution: { status: 'conflicting', deal },
      warnings: [],
    }
  }

  const propertyId = resolvedPropertyId ?? deal?.propertyId
  const intents = deriveIntents(event, personResolution.personId, propertyId)

  return {
    status: 'ready',
    normalizedEvent: event,
    interactionInput: interactionInput(
      event,
      personResolution.personId,
      propertyId,
      deal?.id,
    ),
    personResolution,
    propertyResolution,
    dealResolution,
    ...intents,
    warnings: [],
  }
}
