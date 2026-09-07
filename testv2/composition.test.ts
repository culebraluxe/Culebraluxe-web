import { test } from 'node:test'
import assert from 'node:assert/strict'
import { composeCoreServices, type CoreServiceRepositories } from '../services/composition'
import type { FirmDto, FirmRepository } from '../services/firm'
import type {
  PropertyDto,
  PropertyForPersonDto,
  PropertyRepository,
  UpsertPropertyForPersonRequest,
} from '../services/property'
import type { ContractDto, ContractRepository } from '../services/contract'
import type { SecurityRepository } from '../services/security'
import type { ShowingDto, ShowingRepository } from '../services/showing'
import { context, MemoryPersonRepository, principal } from './test-support'

// ---------------------------------------------------------------------------
// TESTV2 — Composition / cross-service wiring through composeCoreServices.
// ---------------------------------------------------------------------------

const actor = { id: 'u-1', kind: 'user' as const }

const propertyId = 'pr1'

function propertyDto(): PropertyDto {
  return {
    id: propertyId,
    displayName: 'Ocean View Home',
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
}

const firmRepo: FirmRepository = {
  async get() {
    return null
  },
  async findByName() {
    return null
  },
  async upsert(_r: { name: string; firmId?: string }): Promise<FirmDto> {
    throw new Error('unused in composition test')
  },
}
const propertyRepo: PropertyRepository = {
  async get() {
    return propertyDto()
  },
  async findByAddress() {
    return null
  },
  async forPerson(personId: string) {
    return { personId, properties: [], observedAddresses: [] }
  },
  async upsertForPerson(_r: UpsertPropertyForPersonRequest): Promise<PropertyForPersonDto> {
    throw new Error('unused in composition test')
  },
  async setDisplayName(_r: unknown): Promise<PropertyDto> {
    throw new Error('unused in composition test')
  },
  async setStatus(_r: unknown): Promise<PropertyDto> {
    throw new Error('unused in composition test')
  },
}
const contractRepo: ContractRepository = {
  async get() {
    return null
  },
  async createFromForm(request: { contractId: string; contractType: string; formTemplateId: string }): Promise<ContractDto> {
    return {
      id: request.contractId,
      contractType: request.contractType,
      formTemplateId: request.formTemplateId,
      sourceFormInstanceId: null,
      predecessorContractId: null,
      propertyId,
      roles: [],
      facts: {},
      status: 'draft',
      executedAt: null,
      evidenceDocumentId: null,
    }
  },
  async saveDraft(_r: unknown): Promise<ContractDto> {
    throw new Error('unused in composition test')
  },
  async getEffectiveState() {
    return null
  },
  async execute(_r: unknown): Promise<ContractDto> {
    throw new Error('unused in composition test')
  },
}

const securityRepo: SecurityRepository = {
  async resolveProviderSubject() {
    return { kind: 'unmapped' }
  },
  async getPrincipal() {
    return null
  },
}

const showingRepo: ShowingRepository = {
  async get(showingId: string): Promise<ShowingDto | null> {
    return showingId === 'sh1' ? showingDto('sh1') : null
  },
  async saveReport(request: { showingId: string }): Promise<ShowingDto> {
    return showingDto(request.showingId)
  },
}

function showingDto(id: string): ShowingDto {
  return {
    id,
    personId: 'p1',
    propertyId,
    status: 'pending',
    showingDate: null,
    duration: null,
    outcome: null,
    interestScore: null,
    feedback: null,
    followUp: null,
    completedAt: null,
  }
}

const repositories: CoreServiceRepositories = {
  person: new MemoryPersonRepository().seed({ id: 'p1', displayName: 'Ana', status: 'active', archivedAt: null }),
  firm: firmRepo,
  property: propertyRepo,
  contract: contractRepo,
  showing: showingRepo,
  security: securityRepo,
}

const actorContext = context({ actor, principal: principal('USER') })

test('composeCoreServices registers every core domain sorted and always builds security + showing', async () => {
  const { registry, security, showing } = composeCoreServices(repositories)
  assert.ok(security, 'security is never null in a full composition')
  assert.ok(showing, 'showing is a first-class composed domain')
  assert.deepEqual(registry.list().map((s) => s.domain), [
    'contract',
    'firm',
    'person',
    'property',
    'security',
    'showing',
  ])
})

test('the composed registry routes a full person envelope to the seeded repository', async () => {
  const { registry } = composeCoreServices(repositories)
  const res = await registry.dispatch('person', {
    operation: 'person.get',
    payload: { personId: 'p1' },
    context: actorContext,
  })
  assert.equal(res.ok, true)
  if (res.ok) {
    const value = res.value as { displayName?: string } | null
    assert.equal(value?.displayName, 'Ana')
  }
})

test('the composed registry rejects envelopes for an unregistered domain', async () => {
  const { registry } = composeCoreServices(repositories)
  const res = await registry.dispatch('media', {
    operation: 'media.list',
    payload: {},
    context: actorContext,
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'SERVICE_NOT_FOUND')
})

test('contract.createFromForm resolves its property dependency through the router and persists a draft', async () => {
  const { registry } = composeCoreServices(repositories)
  const res = await registry.dispatch('contract', {
    operation: 'contract.createFromForm',
    payload: {
      contractId: 'c1',
      contractType: 'listing',
      formTemplateId: 'LISTING-01',
      propertyId,
      roles: [],
      facts: {},
    },
    context: actorContext,
  })
  assert.equal(res.ok, true)
  if (res.ok) {
    const value = res.value as ContractDto
    assert.equal(value.status, 'draft')
    assert.equal(value.propertyId, propertyId)
  }
})
