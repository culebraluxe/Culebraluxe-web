// ---------------------------------------------------------------------------
// TESTV2 — Story 2: authorize is real, the stub is explicit, and audit carries
// the decision. GUEST (missing principal) may query but never run commands;
// contract.execute is the first enforced (non-stub) rule requiring
// BUSINESS_POWER_USER or ROOT.
// ---------------------------------------------------------------------------
import { test } from 'node:test'
import assert from 'node:assert/strict'

import { ContractService } from '../services/contract'
import type { ContractDto, ContractRepository } from '../services/contract'
import { AuthorizationService, StaticAuthorizationPolicyProvider } from '../services/entitlement'
import type { ServiceAuditEvent, ServiceDomainEvent } from '../services/core'
import { context, principal } from './test-support'

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
  saveDraftCalls = 0
  seed(dto: ContractDto): this {
    this.map.set(dto.id, dto)
    return this
  }
  async get(contractId: string): Promise<ContractDto | null> {
    return this.map.get(contractId) ?? null
  }
  async createFromForm(_request: { contractId: string; contractType: string; formTemplateId: string }): Promise<ContractDto> {
    throw new Error('not used in authorization spec')
  }
  async saveDraft(request: { contractId: string }): Promise<ContractDto> {
    this.saveDraftCalls += 1
    const existing = this.map.get(request.contractId)
    if (existing) return existing
    throw new Error('unexpected saveDraft')
  }
  async getEffectiveState() {
    return null
  }
  async execute(request: { contractId: string; evidenceDocumentId?: string }): Promise<ContractDto> {
    const existing = this.map.get(request.contractId)
    if (!existing) throw new Error('not found')
    const executed: ContractDto = {
      ...existing,
      status: 'executed',
      executedAt: '2026-01-01T00:00:00Z',
      evidenceDocumentId: request.evidenceDocumentId ?? null,
    }
    this.map.set(request.contractId, executed)
    return executed
  }
}

type Harness = {
  service: ContractService
  repository: MemoryContractRepository
  audits: ServiceAuditEvent[]
  events: ServiceDomainEvent[]
}

/** Infra with the REAL EntitlementService stub + full audit/event capture. */
function entitlementHarness(): Harness {
  const audits: ServiceAuditEvent[] = []
  const events: ServiceDomainEvent[] = []
  const repository = new MemoryContractRepository()
  const service = new ContractService(repository, {
    authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
    audit: { record: async (a) => audits.push(a) },
    events: { emit: async (e) => events.push(e) },
  })
  return { service, repository, audits, events }
}

test('GUEST command is FORBIDDEN, the handler is never invoked, and audit carries the open-stub decision', async () => {
  const { service, repository, audits } = entitlementHarness()
  const res = await service.execute({
    operation: 'contract.saveDraft',
    payload: { contractId: 'c1', contractType: 'listing', formTemplateId: 'LISTING-01', propertyId: 'pr1', roles: [], facts: {} },
    context: context({ actor }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'FORBIDDEN')
  assert.equal(repository.saveDraftCalls, 0, 'handler must not run for a denied command')
  const failure = audits.find((a) => a.operation === 'contract.saveDraft' && a.outcome === 'failure')
  assert.ok(failure, 'a failure audit exists')
  assert.equal(failure?.errorCode, 'FORBIDDEN')
  assert.equal(failure?.authorization?.policyId, 'default:guest.command-deny')
  assert.equal(failure?.authorization?.mode, 'enforced')
  assert.equal(failure?.authorization?.allowed, false)
})

test('GUEST query is allowed under the open stub', async () => {
  const { service, repository } = entitlementHarness()
  repository.seed(contractDto('c1', 'draft'))
  const res = await service.execute({
    operation: 'contract.get',
    payload: { contractId: 'c1' },
    context: context({ actor }),
  })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value?.id, 'c1')
})

test('USER cannot execute a contract (enforced rule, not open-stub)', async () => {
  const { service, repository, audits } = entitlementHarness()
  repository.seed(contractDto('c1', 'draft'))
  const res = await service.execute({
    operation: 'contract.execute',
    payload: { contractId: 'c1' },
    context: context({ actor, principal: principal('USER') }),
  })
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'FORBIDDEN')
  const failure = audits.find((a) => a.operation === 'contract.execute' && a.outcome === 'failure')
  assert.equal(failure?.authorization?.mode, 'enforced')
  assert.equal(failure?.authorization?.policyId, 'rule:contract.execute')
})


test('BUSINESS_POWER_USER executes a contract: repository reached, event emitted, audit stamped', async () => {
  const { service, repository, audits, events } = entitlementHarness()
  repository.seed(contractDto('c1', 'draft'))
  const res = await service.execute({
    operation: 'contract.execute',
    payload: { contractId: 'c1', evidenceDocumentId: 'doc-9' },
    context: context({ actor, principal: principal('BUSINESS_POWER_USER') }),
  })
  assert.equal(res.ok, true)
  if (res.ok) {
    assert.equal(res.value.status, 'executed')
    assert.equal(res.value.evidenceDocumentId, 'doc-9')
  }
  assert.ok(events.some((e) => e.type === 'contract.executed'), 'contract.executed emitted')
  const success = audits.find((a) => a.operation === 'contract.execute' && a.outcome === 'success')
  assert.ok(success, 'success audit exists')
  assert.equal(success?.authorization?.mode, 'enforced')
  assert.equal(success?.authorization?.policyId, 'rule:contract.execute')
  assert.equal(success?.authorization?.allowed, true)
})

test('ROOT has god access and may execute a contract', async () => {
  const { service, repository } = entitlementHarness()
  repository.seed(contractDto('c1', 'draft'))
  const res = await service.execute({
    operation: 'contract.execute',
    payload: { contractId: 'c1' },
    context: context({ actor, principal: principal('ROOT') }),
  })
  assert.equal(res.ok, true)
  if (res.ok) assert.equal(res.value.status, 'executed')
})

