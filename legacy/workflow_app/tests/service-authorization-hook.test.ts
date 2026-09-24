// ---------------------------------------------------------------------------
// The authorization hook is a property of the ABSTRACT service, and this is the
// fence around that claim.
//
// `BaseService` runs authorization before every operation handler and treats a
// missing port as a boot-configuration error rather than a silent allow. Two
// things follow, and both are asserted here rather than assumed:
//
//   1  NO SERVICE CAN EXECUTE WITHOUT A DECISION. A kernel composed without an
//      authorization port refuses every operation — including the ones whose
//      handlers would have been happy to run.
//   2  THE KERNEL CAN ONLY ASK FOR ACTIONS RUST CAN DECIDE. Operations without an
//      explicit `authorization` fall back to their operation name, and the Rust
//      authorize endpoint refuses an action its catalog does not contain. A
//      mismatch would therefore turn a working operation into a 400 the moment
//      the port started asking, so the two lists are compared here instead.
//
// Point 2 is the one that would otherwise be discovered in production, by a user:
// the caller would see "not authorized" for a feature nobody changed.
// ---------------------------------------------------------------------------

import { test } from 'node:test'
import assert from 'node:assert/strict'
import { readFile } from 'node:fs/promises'

import { SqlContractRepository } from '@/legacy/db/contract-service-repository'
import { SqlFirmRepository } from '@/legacy/db/firm-service-repository'
import { SqlPersonRepository } from '@/legacy/db/person-service-repository'
import { SqlProjectRepository } from '@/legacy/db/project-service-repository'
import { SqlPropertyRepository } from '@/legacy/db/property-service-repository'
import { RustSecurityRepository } from '@/legacy/db/security-service-repository'
import { SqlShowingRepository } from '@/legacy/db/showing-service-repository'
import { SqlWbsRepository } from '@/legacy/db/wbs-service-repository'
import { composeCoreServices } from '@/legacy/services/composition'
import { SecurityService } from '@/legacy/services/security'

/** The same composition the login seam uses, so this fence covers what actually ships. */
function repositories() {
  return {
    person: new SqlPersonRepository(),
    firm: new SqlFirmRepository(),
    property: new SqlPropertyRepository(),
    contract: new SqlContractRepository(),
    showing: new SqlShowingRepository(),
    security: new RustSecurityRepository(),
    wbs: new SqlWbsRepository(),
    project: new SqlProjectRepository(),
  }
}

type ServiceLike = {
  domain: string
  capabilities: () => { name: string; authorization?: string }[]
  execute: (envelope: unknown) => Promise<{ ok: boolean; error?: { code: string } }>
}

/** Services only: the composition also contains undefined slots and the registry. */
function servicesOf(kernel: object): ServiceLike[] {
  return Object.values(kernel).filter(
    (value): value is ServiceLike =>
      typeof (value as ServiceLike | undefined)?.capabilities === 'function',
  )
}

const systemContext = {
  actor: { id: null, kind: 'system' },
  correlationId: 'test-correlation',
}

/** The action names the Rust authorize endpoint's catalog contains. */
async function rustCatalog(): Promise<Set<string>> {
  const source = await readFile(
    new URL('../../../rust/server/src/security/entitlement_catalog.rs', import.meta.url),
    'utf8',
  )
  const body = source.slice(source.indexOf('ACTIONS'))
  return new Set([...body.matchAll(/\("([a-z0-9_.]+)", "(?:query|command)"\)/g)].map((m) => m[1]))
}

test('a service without an authorization port refuses, it does not run', async () => {
  // Constructed DIRECTLY, bypassing the composition default, because that is what
  // the rule is about: the abstract service must not execute for want of a
  // decision. A missing port is a boot-configuration error, never a silent allow.
  const repository = {
    async resolveProviderSubject() {
      return { kind: 'unmapped' as const }
    },
    async getPrincipal() {
      return null
    },
  }
  const service = new SecurityService(repository, {})
  const [capability] = service.capabilities()

  const result = await service.execute({
    operation: capability.name,
    payload: {},
    context: systemContext,
  })
  assert.equal(result.ok, false, 'an operation with no decision source is refused')
  assert.equal(result.error?.code, 'AUTHORIZATION_UNAVAILABLE')
})

test('the composed kernel runs the hook for EVERY service, before any handler', async () => {
  // A port that refuses everything: if any service skipped the hook, its handler
  // would run and this would come back as a business error or a success instead.
  const denying = {
    mode: 'enforced' as const,
    authorize: async () => ({
      allowed: false,
      reason: 'fence: refusing everything on purpose',
      policyId: 'fence:deny',
      mode: 'enforced' as const,
    }),
  }
  const kernel = composeCoreServices(repositories(), { authorization: denying })
  const services = servicesOf(kernel)

  assert.ok(services.length >= 8, `the kernel composed its services (${services.length})`)
  for (const service of services) {
    const [capability] = service.capabilities()
    assert.ok(capability, `${service.domain} exposes at least one operation`)
    const result = await service.execute({
      operation: capability.name,
      payload: {},
      context: systemContext,
    })
    assert.equal(result.ok, false, `${service.domain}.${capability.name} must be refused`)
    assert.equal(
      result.error?.code,
      'FORBIDDEN',
      `${service.domain}.${capability.name} was refused BY the hook, before its handler`,
    )
  }
})

test('every action the composed kernel asks for is a catalogued Rust action', async () => {
  const catalog = await rustCatalog()
  assert.ok(catalog.size > 20, `the Rust catalog parsed (${catalog.size} actions)`)

  const asked = new Set<string>()
  for (const service of servicesOf(composeCoreServices(repositories()))) {
    for (const capability of service.capabilities()) {
      asked.add(capability.authorization ?? capability.name)
    }
  }

  const undecidable = [...asked].filter((action) => !catalog.has(action)).sort()
  assert.deepEqual(
    undecidable,
    [],
    'the kernel would ask Rust to decide actions its catalog does not contain — give these ' +
      'operations an explicit `authorization` naming a catalogued action, or add them to the catalog',
  )
})
