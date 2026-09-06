import { test } from 'node:test'
import assert from 'node:assert/strict'
import { ContractService } from '../services/contract'
import type { ContractDto, ContractRepository } from '../services/contract'
import { capturingInfrastructure, context } from './test-support'

// ---------------------------------------------------------------------------
// TESTV2 — Contract service envelope tests (form -> draft -> execute).
// ---------------------------------------------------------------------------

const actor = { id: 'u-1', kind: 'user' as const }

function contractDto(id: string, status: string): ContractDto {
  return {
    id,
    contractType: 'listing',
    formTemplateId: 'LISTING-01',
    sourceFormInstanceId: null,
    predecessorContractId: null,
    propertyId: 'pr1',
    roles: [],
    facts: {},
    status,
    executedAt: null,
    evidenceDocumentId: null,
  }
}

class MemoryContractRepository implements ContractRepository {
  private readonly map = new Map<string, ContractDto>()
  seed(dto: ContractDto): this {
    this.map.set(dto.id, dto)
    return this
  }
  async get(contractId: string): Promise<ContractDto | null> {
    return this.map.get(contractId) ?? null
  }
  async createFromForm(request: { contractId: string; contractType: string; formTemplateId: string; propertyId: string }): Promise<ContractDto> {
    const dto = contractDto(request.contractId, 'draft')
    this.map.set(request.contractId, dto)
    return dto
  }
  async saveDraft(request: { contractId: string }): Promise<ContractDto> {
    const dto = contractDto(request.contractId, 'draft')
    this.map.set(request.contractId, dto)
    return dto
  }
  async getEffectiveState(contractId: string) {
    const existing = this.map.get(contractId)
    return existing ? { contractId, facts: existing.facts, sourceContractIds: [] } : null
  }
  async execute(request: { contractId: string; evidenceDocumentId?: string }): Promise<ContractDto> {
    const existing = this.map.get(request.contractId)
    if (!existing) throw new Error('not found')
    const executed = { ...existing, status: 'executed', executedAt: '2026-01-01T00:00:00Z', evidenceDocumentId: request.evidenceDocumentId ?? null }
    this.map.set(request.contractId, executed)
    return executed
  }
}

test('contract.createFromForm fails cleanly without a cross-domain router (SERVICE_ROUTER_UNAVAILABLE)', async () => {
  // createFromForm validates its Property through the owning service, so it is
  // inert (never touches the repo) when no ServiceRouter is wired. The composed
  // happy path is covered in composition.test.ts.
  const repo = new MemoryContractRepository()
  const service = new ContractService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({
    operation: 'contract.createFromForm',
    payload: { contractId: 'c1', contractType: 'listing', formTemplateId: 'LISTING-01', propertyId: 'pr1', roles: [], facts: {} },
    context: context({ actor }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'SERVICE_ROUTER_UNAVAILABLE')
})

test('contract.execute transitions a draft to executed with an evidence document', async () => {
  const repo = new MemoryContractRepository().seed(contractDto('c1', 'draft'))
  const service = new ContractService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({
    operation: 'contract.execute',
    payload: { contractId: 'c1', evidenceDocumentId: 'doc-9' },
    context: context({ actor }),
  })
  assert.equal(res.ok, true)
  if (res.ok) {
    assert.equal(res.value.status, 'executed')
    assert.equal(res.value.evidenceDocumentId, 'doc-9')
    assert.ok(res.value.executedAt, 'executedAt is set on execute')
  }
})

test('contract.getEffectiveState returns the facts for a draft', async () => {
  const repo = new MemoryContractRepository().seed({ ...contractDto('c1', 'draft'), facts: { term: '90' } })
  const service = new ContractService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'contract.getEffectiveState', payload: { contractId: 'c1' }, context: context({ actor }) })
  assert.equal(res.ok, true)
  if (res.ok) assert.deepEqual(res.value?.facts, { term: '90' })
})
