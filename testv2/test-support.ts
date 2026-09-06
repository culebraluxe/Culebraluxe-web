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

  async setDisplayName(request: SetPersonDisplayNameRequest): Promise<PersonDto> {
    const existing = this.persons.get(request.personId)
    if (!existing) throw new Error(`Person not found: ${request.personId}`)
    const updated: PersonDto = { ...existing, displayName: request.displayName }
    this.persons.set(request.personId, updated)
    return updated
  }

  async attachIdentity(request: AttachPersonIdentityRequest): Promise<PersonIdentityDto> {
    const key = MemoryPersonRepository.identityKey(request.identity)
    const existing = this.identityOwner.get(key)
    if (existing && existing !== request.personId) {
      throw new Error('identity already owned by another Person')
    }
    this.identityOwner.set(key, request.personId)
    return { ...request.identity }
  }
}

/** Build a PersonService wired to a memory repo + capturing infrastructure. */
export function makePersonHarness() {
  const repository = new MemoryPersonRepository()
  const infra = capturingInfrastructure()
  const service = new PersonService(repository, infra.infrastructure)
  return { repository, service, infra }
}

export function newContext(actor?: ServiceActor): ServiceContext {
  return context({ actor: actor ?? { id: 'u-1', kind: 'user' } })
}
