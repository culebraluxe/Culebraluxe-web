// PROOF: the corridor — the FORM is the input, the CONTRACT is the artifact.
// (docs/REAL-ESTATE-TRANSACTION-DESIGN.md section 1 and build order 7.1-7.2.)
//
// Proves the wiring that made the bridge real rather than theoretical:
//   planContractFromForm(form) -> contract.saveDraft -> the form's contract_id
// It is idempotent on purpose: saving the SAME form twice must UPDATE one
// artifact, never create a second.
//
// Usage: node --env-file=.env.local --import tsx scripts/verify-listing-contract-bridge.ts [formInstanceId]
// Writes to a real draft Listing form in DEV, so it is not read-only.

import { randomUUID } from 'node:crypto'

import { sql } from '../db/client'
import { updateFormInstance } from '../db/form-service-repository'
import { composeCoreServices } from '../services/composition'
import {
  AuthorizationService,
  StaticAuthorizationPolicyProvider,
} from '../services/entitlement/authorization-service'
import { SqlContractRepository } from '../db/contract-service-repository'
import { SqlFirmRepository } from '../db/firm-service-repository'
import { SqlPersonRepository } from '../db/person-service-repository'
import { SqlPropertyRepository } from '../db/property-service-repository'
import { CONTRACT_OPERATIONS } from '../services/contract'
import { planContractFromForm } from '../lib/forms/contract-from-form'
import { getTemplate } from '../lib/forms/template-registry'

const FORM_ID = process.argv[2] ?? '81a49523-1944-4c5d-9d7a-b9dc343912bc'

/**
 * Composed here rather than imported from lib/service-runtime (server-only: it
 * cannot load in a plain tsx script). The four services the bridge touches —
 * contract, property, person, firm — are the real repositories, so saving a
 * contract validates its subject property and its parties exactly as the app does.
 */
const services = composeCoreServices(
  {
    contract: new SqlContractRepository(),
    property: new SqlPropertyRepository(),
    person: new SqlPersonRepository(),
    firm: new SqlFirmRepository(),
    showing: null as never,
    security: null as never,
    wbs: null as never,
    project: null as never,
  },
  { authorization: new AuthorizationService(new StaticAuthorizationPolicyProvider()) },
)

const context = () => ({
  actor: { id: 'probe-user', kind: 'user' as const },
  correlationId: 'probe',
  principal: {
    appUserId: 'probe-user',
    level: 'BUSINESS_POWER_USER' as const,
    roleCodes: ['owner'],
  },
})

async function main() {
  const rows = (await sql`
    select id, template_id, template_version, person_id, property_id, contract_id, status, field_values
    from document_form_instance where id = ${FORM_ID} limit 1
  `) as Array<{
    id: string
    template_id: string
    template_version: number
    person_id: string | null
    property_id: string | null
    contract_id: string | null
    status: string
    field_values: Record<string, string>
  }>
  const form = rows[0]
  if (!form) throw new Error(`form not found: ${FORM_ID}`)
  console.log('form:', form.id, form.template_id, 'v' + form.template_version, '| person:', !!form.person_id, '| property:', !!form.property_id)

  const template = getTemplate(form.template_id, form.template_version)
  if (!template) throw new Error('template not found')
  if (!form.property_id) throw new Error('form has no property')

  // Exactly what the bridge does: reuse the form's stored artifact id, so a
  // re-run (or a re-save) UPDATES one contract instead of creating another.
  const contractId = form.contract_id ?? randomUUID()
  const plan = planContractFromForm({
    template,
    values: form.field_values,
    contractType: 'listing_agreement',
    contractId,
    propertyId: form.property_id,
    parties: form.person_id ? { SELLER: [{ personId: form.person_id }] } : {},
    sourceFormInstanceId: form.id,
  })
  console.log('plan:', JSON.stringify({ roles: plan.roles.map(r => `${r.role}:${r.identityId ? 'linked' : 'unfilled'}`), facts: Object.keys(plan.facts) }))

  const roles = plan.roles.filter((r) => r.identityId).map((r) =>
    r.kind === 'firm'
      ? { kind: 'firm' as const, firmId: r.identityId as string, roleCode: r.role, ordinal: r.ordinal, snapshotName: r.snapshotName }
      : { kind: 'person' as const, personId: r.identityId as string, roleCode: r.role, ordinal: r.ordinal, snapshotName: r.snapshotName },
  )
  const payload = {
    contractId,
    contractType: plan.contractType,
    formTemplateId: plan.formTemplateId,
    sourceFormInstanceId: plan.sourceFormInstanceId,
    predecessorContractId: plan.predecessorContractIds[0] ?? null,
    propertyId: plan.propertyId as string,
    roles,
    facts: plan.facts,
  }

  const first = await services.contract.execute({
    operation: CONTRACT_OPERATIONS.SAVE_DRAFT,
    payload,
    context: context(),
  })
  console.log('saveDraft #1:', first.ok ? 'OK' : 'FAILED ' + first.error.code + ' ' + first.error.message)

  // second save must UPDATE the same artifact, not create a second one
  const second = await services.contract.execute({
    operation: CONTRACT_OPERATIONS.SAVE_DRAFT,
    payload: { ...payload, facts: { ...plan.facts, reSaved: 'yes' } },
    context: context(),
  })
  console.log('saveDraft #2:', second.ok ? 'OK' : 'FAILED ' + second.error.code)

  const count = (await sql`
    select count(*)::int as n from contract where source_form_instance_id = ${form.id}
  `) as Array<{ n: number }>
  console.log('contracts for this form:', count[0]?.n, count[0]?.n === 1 ? '(one artifact — no duplicate)' : `(EXPECTED 1, GOT ${count[0]?.n})`)

  // the form's artifact link round-trips (the column the bridge writes back)
  const linked = await updateFormInstance(form.id, { contractId })
  console.log('form.contract_id wrote back:', linked?.contractId === contractId ? 'YES' : 'NO -> ' + linked?.contractId)

  const back = (await sql`
    select c.id, c.contract_type, c.status, c.facts, f.contract_id
    from contract c join document_form_instance f on f.contract_id = c.id
    where c.id = ${contractId}
  `) as Array<{ id: string; contract_type: string; status: string; facts: Record<string, unknown>; contract_id: string }>
  const row = back[0]
  console.log('read back:', row ? `${row.contract_type} ${row.status} | facts: ${Object.keys(row.facts).length} | linked: ${row.contract_id === contractId}` : 'MISSING')
  console.log('reSaved fact present:', JSON.stringify(row?.facts).includes('reSaved') ? 'YES (second save updated it)' : 'NO')
}

void main().then(() => process.exit(0)).catch((error) => {
  console.error(error)
  process.exit(1)
})
