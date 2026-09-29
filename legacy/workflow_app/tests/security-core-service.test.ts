import { test } from 'node:test'
import assert from 'node:assert/strict'

import { AuthorizationService } from '@/legacy/services/entitlement'
import {
  SECURITY_OPERATIONS,
  SecurityService,
  hasSecurityLevel,
  resolveSecurityLevel,
  type SecurityRepository,
} from '@/legacy/services/security'
import type { ActingUser } from '@/lib/auth/types'

function actor(roleCodes: string[]): ActingUser {
  return {
    appUserId: 'user-1',
    displayName: 'Security Test',
    email: 'security@example.test',
    accountType: 'internal',
    roleCodes,
    authorityCodes: ['portal.read'],
    personId: null,
  }
}

const context = {
  actor: { id: 'caller-1', kind: 'user' as const },
  correlationId: 'security-test',
}

test('SECURITY-CORE-01: current and legacy role codes collapse into four levels', () => {
  assert.equal(resolveSecurityLevel(['root']), 'ROOT')
  assert.equal(resolveSecurityLevel(['business_power']), 'BUSINESS_POWER_USER')
  assert.equal(resolveSecurityLevel(['user']), 'USER')
  assert.equal(resolveSecurityLevel(['ops']), 'USER')
  assert.equal(resolveSecurityLevel(['guest']), 'GUEST')

  assert.equal(resolveSecurityLevel(['owner']), 'ROOT')
  assert.equal(resolveSecurityLevel(['agent']), 'BUSINESS_POWER_USER')
  assert.equal(resolveSecurityLevel(['viewer']), 'USER')
  assert.equal(resolveSecurityLevel(['client']), 'GUEST')
})

test('SECURITY-CORE-01: highest assigned level wins and unknown roles fail closed to guest', () => {
  assert.equal(
    resolveSecurityLevel(['guest', 'business_power', 'user']),
    'BUSINESS_POWER_USER',
  )
  assert.equal(resolveSecurityLevel(['something-new']), 'GUEST')
  assert.equal(resolveSecurityLevel([]), 'GUEST')

  assert.equal(hasSecurityLevel('ROOT', 'BUSINESS_POWER_USER'), true)
  assert.equal(hasSecurityLevel('USER', 'BUSINESS_POWER_USER'), false)
  assert.equal(hasSecurityLevel('GUEST', 'GUEST'), true)
})

test('SECURITY-CORE-01: SecurityService resolves exact mapped identity and level', async () => {
  const mappedActor = actor(['business_power'])
  const repository: SecurityRepository = {
    async resolveProviderSubject(provider, providerSubject) {
      assert.equal(provider, 'google')
      assert.equal(providerSubject, 'google-subject-1')
      return { kind: 'known', actingUser: mappedActor }
    },
    async getPrincipal(appUserId) {
      assert.equal(appUserId, mappedActor.appUserId)
      return mappedActor
    },
  }

  const service = new SecurityService(repository, {
    authorization: new AuthorizationService(async () => ({ allowed: true, reason: 'test wiring: permit', policyId: 'test:permit', mode: 'enforced' })),
  })

  const resolved = await service.execute({
    operation: SECURITY_OPERATIONS.RESOLVE_IDENTITY,
    payload: { provider: 'google', providerSubject: 'google-subject-1' },
    context,
  })

  assert.equal(resolved.ok, true)
  if (!resolved.ok) return
  assert.equal(resolved.value.kind, 'known')
  if (resolved.value.kind !== 'known') return
  assert.equal(resolved.value.principal.actingUser.appUserId, 'user-1')
  assert.equal(resolved.value.principal.level, 'BUSINESS_POWER_USER')

  const principal = await service.execute({
    operation: SECURITY_OPERATIONS.GET_PRINCIPAL,
    payload: { appUserId: 'user-1' },
    context,
  })
  assert.equal(principal.ok, true)
  if (!principal.ok) return
  assert.equal(principal.value?.level, 'BUSINESS_POWER_USER')
})

test('SECURITY-CORE-01: unmapped identity stays unmapped', async () => {
  const repository: SecurityRepository = {
    async resolveProviderSubject() {
      return { kind: 'unmapped' }
    },
    async getPrincipal() {
      return null
    },
  }

  const service = new SecurityService(repository, {
    authorization: new AuthorizationService(async () => ({ allowed: true, reason: 'test wiring: permit', policyId: 'test:permit', mode: 'enforced' })),
  })
  const result = await service.execute({
    operation: SECURITY_OPERATIONS.RESOLVE_IDENTITY,
    payload: { provider: 'google', providerSubject: 'missing' },
    context,
  })

  assert.deepEqual(
    result.ok ? result.value : null,
    { kind: 'unmapped' },
  )
})

test('SECURITY-CORE-01: the authorization port delegates its decision, it does not hold one', async () => {
  // THE RULES ARE NOT TESTED HERE ANY MORE, because they are not enforced here any more: the port asks the Rust
  // security service (see `AuthorizationService`). The contract.execute floor and the GUEST-without-principal rule
  // moved with them and are asserted in `rust/server/src/security/entitlements.rs`
  // (`the_contract_floor_and_the_guest_default_hold_where_they_are_enforced`).
  //
  // What this test can prove on its own is the DELEGATION: the decision a caller provides is the decision returned,
  // both ways, and the mode is stamped. A denial arriving as a denial is the property BaseService depends on.
  const allow = new AuthorizationService(async () => ({
    allowed: true,
    reason: 'delegate said yes',
    policyId: 'test:allow',
    mode: 'enforced',
  }))
  const deny = new AuthorizationService(async () => ({
    allowed: false,
    reason: 'no matching entitlement',
    policyId: 'rule:security.manage.root',
    mode: 'enforced',
  }))

  const request = {
    domain: 'security',
    action: 'security.role.manage',
    operation: 'security.setUserPrimaryRole',
    kind: 'command',
    actor: context.actor,
  } as const

  const permitted = await allow.authorize({ ...request })
  assert.equal(permitted.allowed, true)
  assert.equal(permitted.policyId, 'test:allow')

  const refused = await deny.authorize({ ...request })
  assert.equal(refused.allowed, false, 'a denial must reach the caller as a denial')
  assert.equal(refused.policyId, 'rule:security.manage.root')
  assert.equal(refused.mode, 'enforced')
})
