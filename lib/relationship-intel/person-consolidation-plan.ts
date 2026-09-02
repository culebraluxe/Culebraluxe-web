// ---------------------------------------------------------------------------
// REL-INTEL — conservative legacy Person split repair planning.
//
// A durable source-person link is an already-established ownership decision.
// When that same source profile contains several deterministic identities that
// legacy loaders split across multiple canonical Person rows, the source-linked
// Person is the survivor ONLY when every identity owned by a candidate loser is
// contained in the authoritative source profile. This repairs old 1:1 loader
// residue without weakening normal ambiguous-match behavior.
// ---------------------------------------------------------------------------

export type SourceProfileAnchor = {
  sourceProfileKey: string
  survivorPersonId: string
  identityKeys: string[]
}

export type PersonIdentitySet = {
  personId: string
  identityKeys: string[]
}

export type PersonConsolidation = {
  sourceProfileKey: string
  survivorPersonId: string
  loserPersonId: string
}

export type PersonConsolidationPlan = {
  consolidations: PersonConsolidation[]
  skippedMultiWinnerLosers: string[]
  skippedPartialIdentityLosers: string[]
}

export function planSourceLinkedPersonConsolidations(
  profiles: SourceProfileAnchor[],
  people: PersonIdentitySet[],
): PersonConsolidationPlan {
  const identitiesByPerson = new Map(
    people.map((person) => [person.personId, new Set(person.identityKeys.filter(Boolean))]),
  )
  const ownersByIdentity = new Map<string, Set<string>>()
  for (const person of people) {
    for (const identity of person.identityKeys.filter(Boolean)) {
      const owners = ownersByIdentity.get(identity) ?? new Set<string>()
      owners.add(person.personId)
      ownersByIdentity.set(identity, owners)
    }
  }

  const proposedByLoser = new Map<string, PersonConsolidation[]>()
  const skippedPartial = new Set<string>()

  for (const profile of profiles) {
    const profileIdentities = new Set(profile.identityKeys.filter(Boolean))
    if (profileIdentities.size === 0) continue

    const candidateOwners = new Set<string>()
    for (const identity of profileIdentities) {
      for (const owner of ownersByIdentity.get(identity) ?? []) candidateOwners.add(owner)
    }
    candidateOwners.delete(profile.survivorPersonId)

    for (const loserPersonId of candidateOwners) {
      const loserIdentities = identitiesByPerson.get(loserPersonId) ?? new Set<string>()
      // The loser must be a pure legacy fragment of THIS authoritative source
      // profile. Any additional identity means we do not have enough evidence
      // to merge it automatically.
      if (
        loserIdentities.size === 0 ||
        [...loserIdentities].some((identity) => !profileIdentities.has(identity))
      ) {
        skippedPartial.add(loserPersonId)
        continue
      }

      const proposed: PersonConsolidation = {
        sourceProfileKey: profile.sourceProfileKey,
        survivorPersonId: profile.survivorPersonId,
        loserPersonId,
      }
      const list = proposedByLoser.get(loserPersonId) ?? []
      list.push(proposed)
      proposedByLoser.set(loserPersonId, list)
    }
  }

  const consolidations: PersonConsolidation[] = []
  const skippedMultiWinner = new Set<string>()
  for (const [loser, proposed] of proposedByLoser) {
    const winners = new Set(proposed.map((candidate) => candidate.survivorPersonId))
    if (winners.size !== 1 || winners.has(loser)) {
      skippedMultiWinner.add(loser)
      continue
    }
    // Same loser can be rediscovered from the same source profile through more
    // than one identity; emit exactly one deterministic consolidation.
    consolidations.push(
      [...proposed].sort((a, b) => a.sourceProfileKey.localeCompare(b.sourceProfileKey))[0],
    )
  }

  consolidations.sort((a, b) =>
    a.survivorPersonId.localeCompare(b.survivorPersonId) ||
    a.loserPersonId.localeCompare(b.loserPersonId),
  )

  return {
    consolidations,
    skippedMultiWinnerLosers: [...skippedMultiWinner].sort(),
    skippedPartialIdentityLosers: [...skippedPartial].sort(),
  }
}
