// ---------------------------------------------------------------------------
// TESTV2 silo — shared service-tier test support.
//
// Provides an in-memory PersonRepository and capturing infrastructure fakes so
// service behavior can be tested deterministically at the envelope boundary
// without a database. New domain tests in this silo add their own repo fakes
// and reuse these captures. The silo imports the REAL services/ source by
// relative path; it never ships with the app.
// ---------------------------------------------------------------------------

import type {
  AuditPort,
  DomainEventPort,
  ServiceActor,
  ServiceContext,
  ServiceDomainEvent,
  ServiceInfrastructure,
} from '../services/core'
import { PersonService, type PersonRepository } from '../services/person'
import type {
  AttachPersonIdentityRequest,
  FindPersonByIdentityRequest,
  PersonDto,
  PersonIdentityDto,
  SetPersonDisplayNameRequest,
} from '../services/person'

export type TestActor = ServiceActor

export function context(over: Partial<ServiceContext> = {}): ServiceContext {
  return {
    actor: { id: 'u-1', kind: 'user' },
    correlationId: 'corr-1',
    ...over,
  }
}

/** Captures events/audits and can toggle authorization decisions. */
export type CapturingInfrastructure = {
  infrastructure: ServiceInfrastructure
  events: ServiceDomainEvent[]
  audits: Array<{ domain: string; operation: string; outcome: string; errorCode?: string }>
  setAuthorized: (allowed: boolean) => void
}

export function capturingInfrastructure(): CapturingInfrastructure {
  const events: ServiceDomainEvent[] = []
  const audits: Array<{ domain: string; operation: string; outcome: string; errorCode?: string }> = []
  let allow = true
  const eventsPort: DomainEventPort = {
    emit: async (event) => {
      events.push(event)
    },
  }
  const auditPort: AuditPort = {
    record: async (a) => {
      audits.push({
        domain: a.domain,
        operation: a.operation,
        outcome: a.outcome,
        errorCode: a.errorCode,
      })
    },
  }
  return {
    infrastructure: {
      events: eventsPort,
      audit: auditPort,
      authorization: { authorize: async () => allow },
    },
    events,
    audits,
    setAuthorized: (allowed: boolean) => {
      allow = allowed
    },
  }
}

/** Deterministic in-memory PersonRepository with identity lookup + ownership. */
export class MemoryPersonRepository implements PersonRepository {
  private readonly persons = new Map<string, PersonDto>()
  private readonly identityOwner = new Map<string, string>() // identityKey -> personId

  seed(dto: PersonDto): this {
    this.persons.set(dto.id, dto)
    return this
  }

  private static identityKey(identity: PersonIdentityDto): string {
    return `${identity.kind}:${identity.value.toLowerCase()}`
  }

  async get(personId: string): Promise<PersonDto | null> {
    return this.persons.get(personId) ?? null
  }

  async findByIdentity(request: FindPersonByIdentityRequest): Promise<PersonDto | null> {
    const owner = this.identityOwner.get(MemoryPersonRepository.identityKey(request.identity))
    return owner ? this.persons.get(owner) ?? null : null
  }

  async setDisplayName(request: SetPersonDisplayNameRequest): Promise<PersonDto | null> {
    const person = this.persons.get(request.personId)
    if (!person) return null
    const next = { ...person, displayName: request.displayName }
    this.persons.set(request.personId, next)
    return next
  }

  async attachIdentity(request: AttachPersonIdentityRequest): Promise<PersonDto | null> {
    const person = this.persons.get(request.personId)
    if (!person) return null
    const key = MemoryPersonRepository.identityKey(request.identity)
    const owner = this.identityOwner.get(key)
    if (owner && owner !== request.personId) {
      throw new Error(`identity already belongs to ${owner}`)
    }
    this.identityOwner.set(key, request.personId)
    const exists = person.identities.some(
      (identity) => MemoryPersonRepository.identityKey(identity) === key,
    )
    const next = exists
      ? person
      : { ...person, identities: [...person.identities, request.identity] }
    this.persons.set(request.personId, next)
    return next
  }
}

export function personService(
  repo: PersonRepository = new MemoryPersonRepository(),
  infrastructure = capturingInfrastructure(),
): {
  service: PersonService
  repo: PersonRepository
  capture: CapturingInfrastructure
} {
  return {
    service: new PersonService(repo, infrastructure.infrastructure),
    repo,
    capture: infrastructure,
  }
}
