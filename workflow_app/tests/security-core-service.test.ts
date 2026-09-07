import { test } from 'node:test'
import assert from 'node:assert/strict'

import { AuthorizationService, StaticAuthorizationPolicyProvider } from '../../services/entitlement'
import {
  SECURITY_OPERATIONS,
  SecurityService,
  hasSecurityLevel,
  resolveSecurityLevel,
  type SecurityRepository,
} from '../../services/security'
import type { ActingUser } from '../../lib/auth/types'

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
    authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
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
    authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()),
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

test('SECURITY-CORE-01: authorization resolver enforces GUEST reads and the contract.execute rule', async () => {
  const entitlements = new AuthorizationService(new StaticAuthorizationPolicyProvider())
  assert.equal(entitlements.mode, 'enforced')

  const base = { domain: 'contract', actor: context.actor }

  const guestCommand = await entitlements.authorize({
    ...base,
    action: 'contract.write',
    operation: 'contract.createFromForm',
    kind: 'command',
  })
  assert.equal(guestCommand.allowed, false, 'GUEST (missing principal) command denied under the resolver')
  assert.equal(guestCommand.policyId, 'default:guest.command-deny')

  const guestQuery = await entitlements.authorize({
    ...base,
    action: 'contract.read',
    operation: 'contract.get',
    kind: 'query',
  })
  assert.equal(guestQuery.allowed, true, 'GUEST query allowed under the resolver')

  const userExecute = await entitlements.authorize({
    ...base,
    action: 'contract.execute',
    operation: 'contract.execute',
    kind: 'command',
    principal: { appUserId: 'user-1', level: 'USER', roleCodes: ['user'] },
  })
  assert.equal(userExecute.allowed, false, 'contract.execute denied for USER')
  assert.equal(userExecute.mode, 'enforced')

  const bpuExecute = await entitlements.authorize({
    ...base,
    action: 'contract.execute',
    operation: 'contract.execute',
    kind: 'command',
    principal: {
      appUserId: 'user-1',
      level: 'BUSINESS_POWER_USER',
      roleCodes: ['business_power'],
    },
  })
  assert.equal(bpuExecute.allowed, true, 'contract.execute allowed for BUSINESS_POWER_USER')
})
