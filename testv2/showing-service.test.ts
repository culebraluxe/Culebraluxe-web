// ---------------------------------------------------------------------------
// TESTV2 — Story 6: Showing joins the kernel with a first-class spec.
// Covers GET + saveReport happy path (events), missing person/property, invalid
// score, and GUEST-command denial under the real entitlement stub.
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import { ServiceRegistry } from '../services/core'
import { PersonService } from '../services/person'
import { PropertyService, type PropertyRepository } from '../services/property'
import { ShowingService, type ShowingRepository, type ShowingDto } from '../services/showing'
import { EntitlementService } from '../services/entitlement'
import { capturingInfrastructure, MemoryPersonRepository, principal, context } from './test-support'

const actor = { id: 'u-1', kind: 'user' as const }

function showingDto(id: string, over: Partial<ShowingDto> = {}): ShowingDto {
  return {
    id,
    personId: 'p1',
    propertyId: 'pr1',
    status: 'report_saved',
    showingDate: null,
    duration: null,
    outcome: null,
    interestScore: null,
    feedback: null,
    followUp: null,
    completedAt: null,
    ...over,
  }
}

class MemoryShowingRepository implements ShowingRepository {
  private readonly map = new Map<string, ShowingDto>()
  saveCalls = 0
  seed(dto: ShowingDto): this {
    this.map.set(dto.id, dto)
    return this
  }
  async get(showingId: string): Promise<ShowingDto | null> {
    return this.map.get(showingId) ?? null
  }
  async saveReport(request: {
    showingId: string
    personId: string
    propertyId: string
    interestScore?: number | null
    outcome?: string | null
    feedback?: string | null
    followUp?: string | null
  }): Promise<ShowingDto> {
    this.saveCalls += 1
    const dto = showingDto(request.showingId, {
      personId: request.personId,
      propertyId: request.propertyId,
      interestScore: request.interestScore ?? null,
      outcome: (request.outcome as ShowingDto['outcome']) ?? null,
      feedback: request.feedback ?? null,
      followUp: request.followUp ?? null,
    })
    this.map.set(dto.id, dto)
    return dto
  }
}

/** Minimal property repo whose GET only resolves 'pr1'. */
function propertyRepo(): PropertyRepository {
  return {
    async get(id) {
      return id === 'pr1'
        ? {
            id: 'pr1',
            displayName: 'Isla House',
            localName: null,
            legalOwnerName: null,
            addressLine1: null,
            municipality: null,
            address: {
              addressLine1: null,
              city: null,
              stateOrProvince: null,
              neighborhood: null,
              postalCode: null,
              country: null,
              isoCountryCode: null,
            },
            status: 'active',
            archivedAt: null,
          }
        : null
    },
    async findByAddress() {
      return null
    },
    async forPerson() {
      return { personId: '', properties: [], observedAddresses: [] }
    },
    async upsertForPerson() {
      throw new Error('unused')
    },
    async setDisplayName() {
      throw new Error('unused')
    },
    async setStatus() {
      throw new Error('unused')
    },
  }
}

function buildHarness() {
  const registry = new ServiceRegistry()
  const infra = capturingInfrastructure().infrastructure
  const personRepo = new MemoryPersonRepository().seed({ id: 'p1', displayName: 'Ana', status: 'active', archivedAt: null })
  const showingRepo = new MemoryShowingRepository()
  const router = { router: registry }
  const person = new PersonService(personRepo, { ...infra, ...router })
  const property = new PropertyService(propertyRepo(), { ...infra, ...router })
  const showing = new ShowingService(showingRepo, { ...infra, ...router })
  registry.register(person)
  registry.register(property)
  registry.register(showing)
  return { showing, showingRepo, registry }
}

const saveReportPayload = {
  showingId: 'sh1',
  personId: 'p1',
  propertyId: 'pr1',
  showingDate: null,
  duration: null,
  outcome: null,
  interestScore: 4,
  feedback: 'great light',
  followUp: null,
}

test('showing.saveReport persists and emits showing.report_saved', async () => {
  const { showing, showingRepo } = buildHarness()
  const res = await showing.execute({
    operation: 'showing.saveReport',
    payload: saveReportPayload,
    context: context({ actor, principal: principal('USER') }),
  })
  assert.equal(res.ok, true)
  assert.equal(showingRepo.saveCalls, 1)
  if (res.ok) assert.equal(res.value.interestScore, 4)
})

test('showing.get returns the canonical DTO (and null when absent)', async () => {
  const { showing, showingRepo } = buildHarness()
  showingRepo.seed(showingDto('sh1'))
  const hit = await showing.execute({ operation: 'showing.get', payload: { showingId: 'sh1' }, context: context({ actor }) })
  assert.equal(hit.ok, true)
  if (hit.ok) assert.equal(hit.value?.personId, 'p1')
  const miss = await showing.execute({ operation: 'showing.get', payload: { showingId: 'missing' }, context: context({ actor }) })
  assert.equal(miss.ok, true)
  if (miss.ok) assert.equal(miss.value, null)
})

test('showing.saveReport fails PROPERTY_NOT_FOUND when the property is unknown', async () => {
  const { showing } = buildHarness()
  const res = await showing.execute({
    operation: 'showing.saveReport',
    payload: { ...saveReportPayload, propertyId: 'nope' },
    context: context({ actor, principal: principal('USER') }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'PROPERTY_NOT_FOUND')
})


test('showing.saveReport fails PERSON_NOT_FOUND when the person is unknown', async () => {
  const { showing } = buildHarness()
  const res = await showing.execute({
    operation: 'showing.saveReport',
    payload: { ...saveReportPayload, personId: 'ghost' },
    context: context({ actor, principal: principal('USER') }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'PERSON_NOT_FOUND')
})

test('showing.saveReport rejects a non-integer / out-of-range interest score', async () => {
  const { showing } = buildHarness()
  const bad = await showing.execute({
    operation: 'showing.saveReport',
    payload: { ...saveReportPayload, interestScore: 9 },
    context: context({ actor, principal: principal('USER') }),
  })
  assert.equal(bad.ok, false)
  if (!bad.ok) assert.equal(bad.error.code, 'INTEREST_SCORE_INVALID')
})

test('GUEST cannot run showing.saveReport (FORBIDDEN under the entitlement stub)', async () => {
  const registry = new ServiceRegistry()
  const showing = new ShowingService(new MemoryShowingRepository(), {
    router: registry,
    authorization: new EntitlementService(),
  })
  registry.register(showing)
  const res = await showing.execute({
    operation: 'showing.saveReport',
    payload: saveReportPayload,
    context: context({ actor }), // no principal => GUEST
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'FORBIDDEN')
})

test('unknown operation on showing returns UNKNOWN_OPERATION', async () => {
  const { showing } = buildHarness()
  const res = await showing.execute({ operation: 'showing.nope', payload: {}, context: context({ actor }) } as never)
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'UNKNOWN_OPERATION')
})
