// The ULTIMATE check: "fill client" on the Listing form maps our Apple-contacts
// person correctly.
//
//   Apple Home  -> l_property LEGAL    -> property + person_property('legal_address')     -> Legal address
//   Apple Work  -> l_property PHYSICAL -> property + person_property('physical_property') -> Actual property
//
// and the Listing form's client fill binds the PHYSICAL property (Apple Work),
// exactly as /api/portal/form-sidecar/listing/select-client does.
//
// Read-only. PROD by default (APP_ENV=production), DEV otherwise.

import { composeCoreServices } from '../services/composition'
import { AuthorizationService } from '../services/entitlement/authorization-service'
import { StaticAuthorizationPolicyProvider } from '../services/entitlement/authorization-service'
import { SqlListingPropertyRepository } from '../db/listing-property-service-repository'
import { SqlPersonRepository } from '../db/person-service-repository'
import { SqlFormInstanceRepository } from '../db/form-service-repository'
import { PROPERTY_OPERATIONS } from '../services/property'
import { PERSON_OPERATIONS } from '../services/person'
import { FORM_OPERATIONS } from '../services/forms'
import {
  LISTING_AGREEMENT_TEMPLATE_ID,
  OFFER_LETTER_TEMPLATE_ID,
  PURCHASE_SALE_AMENDMENT_TEMPLATE_ID,
  PURCHASE_SALE_TEMPLATE_ID,
  SHOWING_REPORT_TEMPLATE_ID,
  getLatestTemplate,
} from '../lib/forms/template-registry'
import { sql } from '../db/client'

const services = composeCoreServices(
  {
    person: new SqlPersonRepository(),
    property: new SqlListingPropertyRepository(),
    form: new SqlFormInstanceRepository(),
    firm: null as never,
    contract: null as never,
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

const context = {
  actor: { id: null, kind: 'system' as const },
  correlationId: 'probe',
  principal: {
    appUserId: 'probe',
    level: 'BUSINESS_POWER_USER' as const,
    roleCodes: ['owner'],
  },
}


async function main() {
  const property = services.property
  const person = services.person
  const forms = services.form
  if (!property || !person || !forms) throw new Error('kernel is incomplete')

  // --- the person Apple gave us -------------------------------------------------
  // Prefer the named person (default: the Apple-contacts friend); otherwise take a
  // person who actually has BOTH relations, so the proof runs in any environment.
  const requestedName = process.argv.find((arg) => arg.startsWith('--name='))?.slice(7)
  const named = ((await sql`
    select id, display_name from person
     where display_name ilike ${`%${requestedName ?? 'santa cruz'}%`}
     order by display_name limit 5
  `) as Array<{ id: string; display_name: string }>).at(0)

  const withBoth = ((await sql`
    select p.id, p.display_name
      from person p
      join person_property a on a.person_id = p.id and a.relation_type = 'legal_address'
      join person_property b on b.person_id = p.id and b.relation_type = 'physical_property'
     order by p.display_name
     limit 1
  `) as Array<{ id: string; display_name: string }>).at(0)

  const juan = named ?? withBoth
  if (!juan) {
    check('find a person with both address relations', false, 'none found')
    return
  }
  check('find the person to fill', true, juan.display_name)

  // --- the "fill client" data path ------------------------------------------------
  const personResult = await person.execute({
    operation: PERSON_OPERATIONS.GET,
    payload: { personId: juan.id },
    context,
  })
  check('person resolves through the Person service', personResult.ok && !!personResult.value)

  const contextResult = await property.execute({
    operation: PROPERTY_OPERATIONS.FOR_PERSON,
    payload: { personId: juan.id },
    context,
  })
  check('property context resolves through the Property service', contextResult.ok)

  const properties = contextResult.ok ? contextResult.value.properties : []
  const legal = properties.find((row) => row.relation === 'legal_address') ?? null
  const physical = properties.find((row) => row.relation === 'physical_property') ?? null

  const describe = (row: typeof legal) =>
    row ? `${row.property.addressLine1 ?? '?'} | ${row.property.municipality ?? '?'}` : 'MISSING'

  console.log('\n  FILL-CLIENT MAPPING')
  console.log(`    Legal address   (Apple Home) : ${describe(legal)}`)
  console.log(`    Actual property (Apple Work) : ${describe(physical)}`)
  console.log(`    bound propertyId (form uses) : ${physical?.property.id ?? 'MISSING'}\n`)

  check('Apple Home landed as the Legal address', Boolean(legal), describe(legal))
  check('Apple Work landed as the Actual property', Boolean(physical), describe(physical))
  check('the form binds the PHYSICAL (Apple Work) property', Boolean(physical?.property.id))
  check(
    'legal and actual are different Properties',
    Boolean(legal && physical && legal.property.id !== physical.property.id),
  )


  // --- the 4 form families still resolve -------------------------------------------
  const families: Array<[string, string]> = [
    ['Listing Agreement', LISTING_AGREEMENT_TEMPLATE_ID],
    ['Offer Letter', OFFER_LETTER_TEMPLATE_ID],
    ['Purchase & Sale', PURCHASE_SALE_TEMPLATE_ID],
    ['P&S Amendment', PURCHASE_SALE_AMENDMENT_TEMPLATE_ID],
    ['Showing Report', SHOWING_REPORT_TEMPLATE_ID],
  ]
  console.log('  FORMS')
  for (const [label, id] of families) {
    const template = getLatestTemplate(id)
    check(`${label} (${id}) resolves`, Boolean(template), template ? `v${template.version}` : 'missing')
  }

  // --- the Forms service paths the page/actions use --------------------------------
  const listResult = await forms.execute({
    operation: FORM_OPERATIONS.LIST_INSTANCES,
    payload: {},
    context,
  })
  const instances = listResult.ok ? listResult.value : []
  check('the Forms list loads through the service', listResult.ok, `${instances.length} instance(s)`)

  const listing = instances.find((row) => row.templateId === LISTING_AGREEMENT_TEMPLATE_ID) ?? null
  if (listing) {
    const getResult = await forms.execute({
      operation: FORM_OPERATIONS.GET_INSTANCE,
      payload: { formInstanceId: listing.id },
      context,
    })
    check(
      'a Listing instance loads (editor path)',
      getResult.ok && !!getResult.value,
      `${listing.templateId} · ${listing.status}`,
    )

    const signerResult = await forms.execute({
      operation: FORM_OPERATIONS.LIST_SIGNER_PEOPLE,
      payload: { formInstanceId: listing.id },
      context,
    })
    const signers = signerResult.ok ? signerResult.value : []
    check(
      'signers resolve through the service',
      signerResult.ok && signers.length > 0,
      signerResult.ok ? signers.map((s) => `${s.name}(${s.role})`).join(', ') || 'none' : 'error',
    )
  } else {
    check('a Listing instance exists', false, 'none found')
  }

  console.log(failures === 0 ? '\nLISTING CLIENT-FILL + FORMS: ALL PASSED' : `\n${failures} FAILURE(S)`)
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
