import { randomUUID } from 'node:crypto'

import type {
  NormalizedIdentityHint,
  NormalizedInboundEvent,
} from './crm-intake-types'
import { resolvePerson } from './crm-person-resolution'
import type {
  IdentityClaim,
  IdentityOwnership,
  PersonCreationPolicy,
  PersonCreationRepositories,
  PersonCreationResult,
} from './crm-person-types'

function identityKey(hint: NormalizedIdentityHint) {
  return `${hint.kind}:${hint.normalizedValue}`
}

function deduplicateHints(hints: NormalizedIdentityHint[]) {
  const seen = new Set<string>()
  return hints.filter((hint) => {
    const key = identityKey(hint)
    if (seen.has(key)) return false
    seen.add(key)
    return true
  })
}

function eligibleClaims(hints: NormalizedIdentityHint[]) {
  const trusted = hints.filter((hint) => hint.evidence !== 'user_supplied')
  const hasCanonicalAnchor = trusted.some(
    (hint) => hint.kind === 'email' || hint.kind === 'phone',
  )
  if (!hasCanonicalAnchor) return []

  let primaryEmailSelected = false
  let primaryPhoneSelected = false

  return trusted.map((hint): IdentityClaim => {
    let isPrimary = false
    if (hint.kind === 'email' && !primaryEmailSelected) {
      isPrimary = true
      primaryEmailSelected = true
    } else if (hint.kind === 'phone' && !primaryPhoneSelected) {
      isPrimary = true
      primaryPhoneSelected = true
    }

    return {
      kind: hint.kind,
      normalizedValue: hint.normalizedValue,
      sourceSystem: hint.kind === 'external' ? hint.sourceSystem : undefined,
      isPrimary,
    }
  })
}

function temporaryDisplayName(
  event: NormalizedInboundEvent,
  claims: IdentityClaim[],
) {
  if (event.actor.displayNameHint) {
    return {
      displayName: event.actor.displayNameHint,
      displayNameSource: 'hint' as const,
    }
  }

  const email = claims.find((claim) => claim.kind === 'email' && claim.isPrimary)
  if (email) {
    return {
      displayName: email.normalizedValue,
      displayNameSource: 'email' as const,
    }
  }

  const phone = claims.find((claim) => claim.kind === 'phone' && claim.isPrimary)
  if (!phone) throw new Error('Eligible identity anchor is missing.')
  return {
    displayName: phone.normalizedValue,
    displayNameSource: 'phone' as const,
  }
}

function result(
  event: NormalizedInboundEvent,
  status: PersonCreationResult['status'],
  values: Partial<Omit<PersonCreationResult, 'status' | 'normalizedEvent'>> = {},
): PersonCreationResult {
  return {
    status,
    normalizedEvent: event,
    claimedIdentities: [],
    unclaimedIdentities: [],
    ...values,
  }
}

async function ownershipsFor(
  hints: NormalizedIdentityHint[],
  repositories: PersonCreationRepositories,
) {
  return Promise.all(
    hints.map(async (hint) => ({
      hint,
      ownership: await repositories.findIdentityOwnership(hint),
    })),
  )
}

function ownershipOutcome(
  event: NormalizedInboundEvent,
  ownerships: Array<{
    hint: NormalizedIdentityHint
    ownership: IdentityOwnership | null
  }>,
) {
  if (ownerships.some(({ ownership }) => ownership?.archived)) {
    return result(event, 'resolution_required', {
      unclaimedIdentities: ownerships.map(({ hint }) => hint),
      reason: 'archived_identity_owner',
    })
  }

  const activePersonIds = new Set(
    ownerships.flatMap(({ ownership }) =>
      ownership ? [ownership.personId] : [],
    ),
  )
  const unclaimedIdentities = ownerships.flatMap(({ hint, ownership }) =>
    ownership ? [] : [hint],
  )

  if (activePersonIds.size > 1) {
    return result(event, 'conflicting', {
      unclaimedIdentities,
      reason: 'identity_conflict',
    })
  }

  if (activePersonIds.size === 1) {
    return result(event, 'resolved_existing', {
      personId: [...activePersonIds][0],
      unclaimedIdentities,
    })
  }

  return null
}

function isUniqueViolation(error: unknown) {
  return (
    typeof error === 'object' &&
    error !== null &&
    'code' in error &&
    error.code === '23505'
  )
}

export async function resolveOrCreateInboundPerson(
  event: NormalizedInboundEvent,
  policy: PersonCreationPolicy,
  repositories: PersonCreationRepositories,
  createId: () => string = randomUUID,
): Promise<PersonCreationResult> {
  const duplicate = await repositories.findInteractionBySourceIdentity(
    event.source.system,
    event.source.externalId,
  )
  if (duplicate) {
    return result(event, 'duplicate', {
      personId: duplicate.personId,
      existingInteractionId: duplicate.id,
    })
  }

  if (event.actor.personId) {
    const explicitPersonExists = await repositories.personExists(
      event.actor.personId,
    )
    if (!explicitPersonExists) {
      return result(event, 'rejected', {
        unclaimedIdentities: deduplicateHints(event.actor.identityHints),
        reason: 'explicit_person_not_found',
      })
    }
  }

  const normalizedEvent = {
    ...event,
    actor: {
      ...event.actor,
      identityHints: deduplicateHints(event.actor.identityHints),
    },
  }
  const personResolution = await resolvePerson(normalizedEvent, repositories)

  if (personResolution.status === 'conflicting') {
    return result(event, 'conflicting', {
      unclaimedIdentities: normalizedEvent.actor.identityHints,
      reason: 'identity_conflict',
    })
  }

  const ownerships = await ownershipsFor(
    normalizedEvent.actor.identityHints,
    repositories,
  )
  const existingOutcome = ownershipOutcome(event, ownerships)
  if (existingOutcome) return existingOutcome

  if (personResolution.status === 'resolved' && personResolution.personId) {
    return result(event, 'resolved_existing', {
      personId: personResolution.personId,
      unclaimedIdentities: normalizedEvent.actor.identityHints,
    })
  }

  if (!policy.allowCreation) {
    return result(event, 'resolution_required', {
      unclaimedIdentities: normalizedEvent.actor.identityHints,
      reason: 'creation_not_allowed',
    })
  }

  const claims = eligibleClaims(normalizedEvent.actor.identityHints)
  if (claims.length === 0) {
    return result(event, 'resolution_required', {
      unclaimedIdentities: normalizedEvent.actor.identityHints,
      reason: 'insufficient_identity_evidence',
    })
  }

  const claimedKeys = new Set(
    claims.map((claim) => `${claim.kind}:${claim.normalizedValue}`),
  )
  const unclaimedIdentities = normalizedEvent.actor.identityHints.filter(
    (hint) => !claimedKeys.has(identityKey(hint)),
  )
  const display = temporaryDisplayName(normalizedEvent, claims)
  const personId = createId()

  try {
    await repositories.createPersonWithIdentities({
      personId,
      displayName: display.displayName,
      role: policy.role,
      identities: claims,
    })
  } catch (error) {
    if (!isUniqueViolation(error)) {
      return result(event, 'rejected', {
        unclaimedIdentities: normalizedEvent.actor.identityHints,
        reason: 'repository_failure',
      })
    }

    const raceOwnerships = await ownershipsFor(
      normalizedEvent.actor.identityHints,
      repositories,
    )
    const raceOutcome = ownershipOutcome(event, raceOwnerships)
    return (
      raceOutcome ??
      result(event, 'resolution_required', {
        unclaimedIdentities: normalizedEvent.actor.identityHints,
        reason: 'race_ownership_unresolved',
      })
    )
  }

  return result(event, 'created', {
    personId,
    ...display,
    claimedIdentities: claims,
    unclaimedIdentities,
  })
}

