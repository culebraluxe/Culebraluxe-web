// PROOF: a form IS a contract. The mapper reads ONLY what the template declares
// and lands every field in exactly one bucket:
//   role (party link + frozen name) · linked field (canonical) · contract fact
//
// Pure and read-only: runs against the real deployed templates.

import { getLatestTemplate } from '../lib/forms/template-registry'
import { planContractFromForm } from '../lib/forms/contract-from-form'

let failures = 0
const check = (name: string, ok: boolean, extra = '') => {
  console.log(`${ok ? 'ok  ' : 'FAIL'} - ${name}${extra ? ' :: ' + extra : ''}`)
  if (!ok) failures++
}

const FORMS: Array<{ id: string; contractType: string }> = [
  { id: 'LISTING-01', contractType: 'listing_agreement' },
  { id: 'PR-PNS', contractType: 'purchase_sale' },
  { id: 'OFFER-01', contractType: 'offer' },
  { id: 'SHOW-RPT', contractType: 'showing_report' },
]

/** The contract role vocabulary the database enforces. */
const CONTRACT_ROLES = new Set([
  'BUYER', 'SELLER', 'BUYER_BROKER', 'SELLER_BROKER', 'SELLER_SPOUSE',
  'BUYER_COUNSEL', 'SELLER_COUNSEL', 'CLOSING_NOTARY', 'LENDER_CONTACT',
  'SELLER_REPRESENTATIVE',
])

function sampleValues(names: readonly string[]): Record<string, string> {
  const values: Record<string, string> = {}
  for (const name of names) values[name] = `sample ${name}`
  return values
}

function main() {
  for (const form of FORMS) {
    const template = getLatestTemplate(form.id)
    if (!template) {
      check(`${form.id} resolves`, false)
      continue
    }
    console.log(`\n  ${form.id} v${template.version} -> ${form.contractType}`)

    const plan = planContractFromForm({
      template,
      values: sampleValues(template.fields.map((field) => field.name)),
      contractType: form.contractType,
      contractId: '00000000-0000-4000-8000-000000000001',
      propertyId: '00000000-0000-4000-8000-000000000002',
      parties: Object.fromEntries(
        template.participants.map((participant) => [
          participant.role,
          [{ personId: '00000000-0000-4000-8000-000000000003' }],
        ]),
      ),
    })

    console.log(`    roles (${plan.roles.length}):`)
    for (const role of plan.roles) {
      console.log(
        `      ${role.role.padEnd(18)} <- ${String(role.nameField).padEnd(18)} name="${role.snapshotName ?? ''}"`,
      )
    }
    console.log(`    facts (${Object.keys(plan.facts).length}): ${Object.keys(plan.facts).join(', ')}`)
    console.log(
      `    linked (${plan.linkedFields.length}): ${plan.linkedFields
        .map((entry) => `${entry.field}=${entry.binding}`)
        .join(', ') || '-'}`,
    )

    // every declared participant became a contract role, from the contract vocabulary
    check(
      `${form.id}: every participant maps to a contract role`,
      plan.roles.map((role) => role.role).every((role) => CONTRACT_ROLES.has(role)),
      [...new Set(plan.roles.map((role) => role.role))].join(' '),
    )
    // every role names the template field it came from, and freezes a name
    check(
      `${form.id}: every role has a name field and a frozen name`,
      plan.roles.every((role) => Boolean(role.nameField) && Boolean(role.snapshotName)),
    )
    // facts never contain a role's name or a canonical link
    const factNames = Object.keys(plan.facts)
    const nameFields = plan.roleNameFields.map((entry) => entry.field)
    check(
      `${form.id}: facts exclude role names and linked fields`,
      factNames.every((name) => !nameFields.includes(name)) &&
        factNames.every((name) => !plan.linkedFields.some((entry) => entry.field === name)),
    )
    // nothing declared is silently dropped
    const accounted = new Set([
      ...factNames,
      ...plan.linkedFields.map((entry) => entry.field),
      ...nameFields,
    ])
    const declared = template.fields.map((field) => field.name)
    check(
      `${form.id}: every declared field is accounted for`,
      declared.every((name) => accounted.has(name)),
      `${declared.length} declared / ${accounted.size} accounted`,
    )
  }

  console.log(failures === 0 ? '\nCONTRACT MAPPER: ALL PASSED' : `\n${failures} FAILURE(S)`)
  process.exit(failures === 0 ? 0 : 1)
}

main()
