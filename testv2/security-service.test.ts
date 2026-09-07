import { test } from 'node:test'
import assert from 'node:assert/strict'
import type { ActingUser } from '../lib/auth/types'
import {
  SecurityService,
  type SecurityRepository,
  type SecurityRepositoryIdentityResolution,
} from '../services/security'
import { resolveSecurityLevel, hasSecurityLevel, type SecurityLevel } from '../services/security'
import { capturingInfrastructure, context } from './test-support'

// ---------------------------------------------------------------------------
// TESTV2 — Security service envelope tests (identity resolution + levels).
// ---------------------------------------------------------------------------

const actor = { id: 'u-1', kind: 'user' as const }

function acting(over: Partial<ActingUser> = {}): ActingUser {
  return {
    appUserId: 'au1',
    displayName: 'Ana',
    email: 'ana@culebraluxe.test',
    accountType: 'internal',
    roleCodes: [],
    authorityCodes: [],
    personId: null,
    ...over,
  }
}

class MemorySecurityRepository implements SecurityRepository {
  private readonly byProvider = new Map<string, SecurityRepositoryIdentityResolution>()
  private readonly byAppUser = new Map<string, ActingUser>()

  resolve(provider: string, subject: string, resolution: SecurityRepositoryIdentityResolution): this {
    this.byProvider.set(`${provider}:${subject}`, resolution)
    if (resolution.kind === 'known') this.byAppUser.set(resolution.actingUser.appUserId, resolution.actingUser)
    return this
  }
  principal(appUserId: string, user: ActingUser): this {
    this.byAppUser.set(appUserId, user)
    return this
  }
  async resolveProviderSubject(provider: string, providerSubject: string) {
    return this.byProvider.get(`${provider}:${providerSubject}`) ?? { kind: 'unmapped' as const }
  }
  async getPrincipal(appUserId: string): Promise<ActingUser | null> {
    return this.byAppUser.get(appUserId) ?? null
  }
}

test('security.resolveIdentity maps a known provider subject to a level-bearing principal', async () => {
  const repo = new MemorySecurityRepository().resolve('google', 'sub-1', {
    kind: 'known',
    actingUser: acting({ roleCodes: ['owner'] }),
  })
  const service = new SecurityService(repo, capturingInfrastructure().infrastructure)
  const res = await service.execute({
    operation: 'security.resolveIdentity',
    payload: { provider: 'google', providerSubject: 'sub-1' },
    context: context({ actor }),
  })
  assert.equal(res.ok, true)
  if (res.ok) {
    assert.equal(res.value.kind, 'known')
    if (res.value.kind === 'known') {
      assert.equal(res.value.principal.actingUser.appUserId, 'au1')
      assert.equal(res.value.principal.level, 'ROOT')
    }
  }
})

test('security.resolveIdentity surfaces unmapped and inactive resolutions untouched', async () => {
  const repo = new MemorySecurityRepository()
    .resolve('google', 'ghost', { kind: 'inactive' })
  const service = new SecurityService(repo, capturingInfrastructure().infrastructure)
  const unmapped = await service.execute({
    operation: 'security.resolveIdentity',
    payload: { provider: 'google', providerSubject: 'nobody' },
    context: context({ actor }),
  })
  assert.equal(unmapped.ok, true)
  if (unmapped.ok) assert.equal(unmapped.value.kind, 'unmapped')

  const inactive = await service.execute({
    operation: 'security.resolveIdentity',
    payload: { provider: 'google', providerSubject: 'ghost' },
    context: context({ actor }),
  })
  assert.equal(inactive.ok, true)
  if (inactive.ok) assert.equal(inactive.value.kind, 'inactive')
})

test('security.getPrincipal returns a principal for a known app_user and null for an unknown one', async () => {
  const repo = new MemorySecurityRepository().principal('au1', acting({ roleCodes: ['user'] }))
  const service = new SecurityService(repo, capturingInfrastructure().infrastructure)
  const known = await service.execute({ operation: 'security.getPrincipal', payload: { appUserId: 'au1' }, context: context({ actor }) })
  assert.equal(known.ok, true)
  if (known.ok) {
    assert.equal(known.value?.actingUser.appUserId, 'au1')
    assert.equal(known.value?.level, 'USER')
  }
  const unknown = await service.execute({ operation: 'security.getPrincipal', payload: { appUserId: 'missing' }, context: context({ actor }) })
  assert.equal(unknown.ok, true)
  if (unknown.ok) assert.equal(unknown.value, null)
})

// --- level module invariants -------------------------------------------------

test('unknown or empty roles fail closed to GUEST, never elevated', () => {
  assert.equal(resolveSecurityLevel([]), 'GUEST')
  assert.equal(resolveSecurityLevel(['nonsense_role']), 'GUEST')
})

test('role codes resolve case-insensitively and collapse to the highest level', () => {
  assert.equal(resolveSecurityLevel(['Owner']), 'ROOT')
  assert.equal(resolveSecurityLevel(['viewer', 'agent']), 'BUSINESS_POWER_USER')
  assert.equal(resolveSecurityLevel(['user', 'client']), 'USER')
})

test('hasSecurityLevel allows equal or higher actual levels', () => {
  const requires: SecurityLevel[] = ['GUEST', 'USER', 'BUSINESS_POWER_USER', 'ROOT']
  const all = requires.map((required) => ({ required, actual: 'ROOT' as const }))
  assert.ok(all.every(({ required, actual }) => hasSecurityLevel(actual, required)))
  assert.equal(hasSecurityLevel('GUEST', 'USER'), false)
})

test('an unknown operation on security returns UNKNOWN_OPERATION', async () => {
  const service = new SecurityService(new MemorySecurityRepository(), capturingInfrastructure().infrastructure)
  const res = await service.execute({ operation: 'security.nope', payload: {}, context: context({ actor }) } as never)
  assert.equal(res.ok, false)
  if (!res.ok) assert.equal(res.error.code, 'UNKNOWN_OPERATION')
})
