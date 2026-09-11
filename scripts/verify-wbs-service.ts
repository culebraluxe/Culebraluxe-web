// PROOF: the WBS domain is wired — work items, the polymorphic entity tie, and
// the transaction-scoped read that makes human work visible on the transaction
// (docs/REAL-ESTATE-TRANSACTION-DESIGN.md sections 6.5 and 7.4).
// Read-only: it exercises the catalogue, the reads, and that commands are guarded.

import { composeCoreServices } from '../services/composition'
import {
  AuthorizationService,
  StaticAuthorizationPolicyProvider,
} from '../services/entitlement/authorization-service'
import { SqlWbsRepository } from '../db/wbs-service-repository'
import { WBS_OPERATIONS } from '../services/wbs'
import { sql } from '../db/client'

const services = composeCoreServices(
  {
    contract: null as never,
    person: null as never,
    firm: null as never,
    property: null as never,
    showing: null as never,
    security: null as never,
    wbs: new SqlWbsRepository(),
    project: null as never,
  },
  { authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()) },
)

let failures = 0
const check = (name: string, ok: boolean, extra = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} - ${name}${extra ? ' :: ' + extra : ''}`)
  if (!ok) failures++
}

const operatorContext = () => ({
  actor: { id: 'probe-user', kind: 'user' as const },
  correlationId: 'probe',
  principal: {
    appUserId: 'probe-user',
    level: 'BUSINESS_POWER_USER' as const,
    roleCodes: ['owner'],
  },
})

const systemContext = () => ({
  actor: { id: null, kind: 'system' as const },
  correlationId: 'probe',
})

async function main() {
  const wbs = services.wbs
  if (!wbs) throw new Error('WBS service was not composed.')

  const catalogue = Object.values(WBS_OPERATIONS)
  const names = (await wbs.capabilities()).map((capability) => capability.name)
  check(
    'the WBS catalogue exposes the item + tie operations',
    catalogue.every((operation) => names.includes(operation)),
    catalogue.join(', '),
  )

  // ---- The tie (design doc 6.5) --------------------------------------------
  // Work items hang off the transaction a process instance runs, using the
  // polymorphic entity link that already carries person/property/contract.
  const transactionId = '00000000-0000-4000-8000-000000000000'
  const tied = await wbs.execute({
    operation: WBS_OPERATIONS.LIST_FOR_ENTITY,
    payload: { type: 'process_instance', id: transactionId },
    context: operatorContext(),
  })
  check(
    'wbs.listForEntity runs through the service for a transaction',
    tied.ok && Array.isArray(tied.value),
    tied.ok ? `${tied.value.length} item(s)` : tied.error.code,
  )
  check(
    'the tie read is filtered to that one entity',
    tied.ok &&
      tied.value.every(
        (item) => item.entity?.type === 'process_instance' && item.entity.id === transactionId,
      ),
  )

  for (const type of ['process_instance', 'contract', 'property', 'person'] as const) {
    const read = await wbs.execute({
      operation: WBS_OPERATIONS.LIST_FOR_ENTITY,
      payload: { type, id: transactionId },
      context: operatorContext(),
    })
    check(
      `wbs.listForEntity accepts entity type ${type}`,
      read.ok,
      read.ok ? `${read.value.length} item(s)` : read.error.code,
    )
  }

  // Prove the tie against REAL data: take an entity that already has work items
  // and read them back through the service.
  const linked = (await sql`
    select entity_type, entity_id, count(*)::int as n
    from wbs_item
    where entity_type is not null and entity_id is not null
    group by entity_type, entity_id
    order by n desc, entity_type
    limit 1
  `) as Array<{ entity_type: string; entity_id: string; n: number }>
  if (linked[0]) {
    const { entity_type, entity_id, n } = linked[0]
    const real = await wbs.execute({
      operation: WBS_OPERATIONS.LIST_FOR_ENTITY,
      payload: { type: entity_type as never, id: entity_id },
      context: operatorContext(),
    })
    check(
      `the tie reads real work items for ${entity_type} (${n} in the table)`,
      real.ok && real.value.length === n,
      real.ok ? `${real.value.length} item(s) via the service` : real.error.code,
    )
    check(
      'every returned item points at that exact entity',
      real.ok &&
        real.value.every((item) => item.entity?.type === entity_type && item.entity.id === entity_id),
    )
  }

  // the project read is unchanged
  const projectItems = await wbs.execute({
    operation: WBS_OPERATIONS.LIST_PROJECT_ITEMS,
    payload: {},
    context: operatorContext(),
  })
  check(
    'wbs.listProjectItems still runs',
    projectItems.ok,
    projectItems.ok ? `${projectItems.value.length} item(s)` : projectItems.error.code,
  )

  // reads on an unknown id are empty, not an error
  const missing = await wbs.execute({
    operation: WBS_OPERATIONS.GET,
    payload: { id: transactionId },
    context: operatorContext(),
  })
  check('wbs.get returns null for an unknown id', missing.ok && missing.value === null)

  // commands stay guarded
  const denied = await wbs.execute({
    operation: WBS_OPERATIONS.COMPLETE,
    payload: { id: transactionId },
    context: systemContext(),
  })
  check(
    'unauthenticated actor is denied wbs.complete',
    !denied.ok && denied.error.code === 'FORBIDDEN',
    denied.ok ? 'ALLOWED (should be denied)' : denied.error.code,
  )

  console.log(failures === 0 ? '\nALL WBS SERVICE PROOFS PASSED' : `\n${failures} FAILURE(S)`)
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
