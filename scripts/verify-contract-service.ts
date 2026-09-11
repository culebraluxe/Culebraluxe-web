// PROOF: the Contract domain is wired and ready for the bridge
// (person -> form -> contract -> document), and the workflow chain is modelled.
// Read-only: the contract table is empty in both environments, so this verifies
// the service catalogue, the reads, and that the write path is guarded.

import { composeCoreServices } from '../services/composition'
import { AuthorizationService } from '../services/entitlement/authorization-service'
import { StaticAuthorizationPolicyProvider } from '../services/entitlement/authorization-service'
import { SqlContractRepository } from '../db/contract-service-repository'
import { CONTRACT_OPERATIONS } from '../services/contract'
import { sql } from '../db/client'

const services = composeCoreServices(
  {
    contract: new SqlContractRepository(),
    person: null as never,
    firm: null as never,
    property: null as never,
    showing: null as never,
    security: null as never,
    wbs: null as never,
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
  const contract = services.contract
  if (!contract) throw new Error('Contract service was not composed.')

  // the workflow-shaped catalogue is present
  const catalogue = Object.values(CONTRACT_OPERATIONS)
  const capabilities = await contract.capabilities()
  const names = capabilities.map((capability) => capability.name)
  check(
    'the Contract catalogue exposes the bridge + workflow operations',
    catalogue.every((operation) => names.includes(operation)),
    catalogue.join(', '),
  )

  // the portfolio read works through the service
  const listResult = await contract.execute({
    operation: CONTRACT_OPERATIONS.LIST,
    payload: {},
    context: operatorContext(),
  })
  check('contract.list runs through the service', listResult.ok, `${listResult.ok ? listResult.value.length : 0} contract(s)`)

  const tableCount = (await sql`select count(*)::int as n from contract`) as Array<{ n: number }>
  check(
    'contract.list agrees with the table',
    listResult.ok && listResult.value.length === tableCount[0]?.n,
    `service ${listResult.ok ? listResult.value.length : '?'} vs table ${tableCount[0]?.n}`,
  )

  // reads on an unknown id are empty, not an error
  const missingId = '00000000-0000-4000-8000-000000000000'
  const getResult = await contract.execute({
    operation: CONTRACT_OPERATIONS.GET,
    payload: { contractId: missingId },
    context: operatorContext(),
  })
  check('contract.get returns null for an unknown id', getResult.ok && getResult.value === null)

  const stateResult = await contract.execute({
    operation: CONTRACT_OPERATIONS.GET_EFFECTIVE_STATE,
    payload: { contractId: missingId },
    context: operatorContext(),
  })
  check(
    'contract.getEffectiveState returns null for an unknown id',
    stateResult.ok && stateResult.value === null,
  )

  // the bridge is guarded: an unauthenticated actor cannot create a contract
  const denied = await contract.execute({
    operation: CONTRACT_OPERATIONS.CREATE_FROM_FORM,
    payload: {
      contractId: missingId,
      contractType: 'listing',
      formTemplateId: 'LISTING-01',
      propertyId: missingId,
      roles: [],
      facts: {},
    },
    context: systemContext(),
  })
  check(
    'unauthenticated actor is denied contract.createFromForm',
    !denied.ok && denied.error.code === 'FORBIDDEN',
    denied.ok ? 'ALLOWED (should be denied)' : denied.error.code,
  )

  console.log(failures === 0 ? '\nALL CONTRACT SERVICE PROOFS PASSED' : `\n${failures} FAILURE(S)`)
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
