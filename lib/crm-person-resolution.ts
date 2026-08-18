import type {
  IntakeRepositories,
  NormalizedInboundEvent,
  PersonResolution,
} from './crm-intake-types'

type PersonResolutionRepositories = Pick<
  IntakeRepositories,
  'personExists' | 'findIdentityMatch'
>

export async function resolvePerson(
  event: NormalizedInboundEvent,
  repositories: PersonResolutionRepositories,
): Promise<PersonResolution> {
  const explicitPersonId = event.actor.personId
  const explicitPersonExists = explicitPersonId
    ? await repositories.personExists(explicitPersonId)
    : false
  const matches = await Promise.all(
    event.actor.identityHints.map((hint) =>
      repositories.findIdentityMatch(hint),
    ),
  )
  const matchedPersonIds = new Set(
    matches.flatMap((match) => (match ? [match.personId] : [])),
  )

  if (explicitPersonId && explicitPersonExists) {
    matchedPersonIds.add(explicitPersonId)
  }

  const conflict =
    matchedPersonIds.size > 1 ||
    Boolean(explicitPersonId && !explicitPersonExists && matches.some(Boolean))
  const resolvedPersonId =
    !conflict && matchedPersonIds.size === 1
      ? [...matchedPersonIds][0]
      : undefined

  return {
    status: conflict
      ? 'conflicting'
      : resolvedPersonId
        ? 'resolved'
        : 'unresolved',
    personId: resolvedPersonId,
    matchedIdentityIds: matches.flatMap((match) =>
      match ? [match.identityId] : [],
    ),
    evidence: event.actor.identityHints.map((hint, index) => ({
      kind: hint.kind,
      normalizedValue: hint.normalizedValue,
      result: matches[index]
        ? conflict
          ? 'conflict'
          : 'matched'
        : 'unmatched',
    })),
  }
}

