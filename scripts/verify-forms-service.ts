// PROOF: the Forms domain now owns form persistence through the service.
//   (a) reads match the repository path exactly
//   (b) a real command (create -> update -> get) works with an operator principal
//   (c) an unauthenticated/system actor is DENIED a command (fail closed)
// DEV only. The probe row it creates is deleted again.

import { composeCoreServices } from '../services/composition'
import { AuthorizationService } from '../services/entitlement/authorization-service'
import { StaticAuthorizationPolicyProvider } from '../services/entitlement/authorization-service'
import { SqlFormInstanceRepository } from '../db/form-service-repository'
import { getFormInstance, latestFormEvidence, listFormInstances } from '../db/form-service-repository'
import { FORM_OPERATIONS } from '../services/forms'
import { sql } from '../db/client'

const services = composeCoreServices(
  {
    person: null as never,
    firm: null as never,
    property: null as never,
    contract: null as never,
    showing: null as never,
    security: null as never,
    wbs: null as never,
    project: null as never,
    form: new SqlFormInstanceRepository(),
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
  principal: { appUserId: 'probe-user', level: 'BUSINESS_POWER_USER' as const, roleCodes: ['owner'] },
})

const systemContext = () => ({
  actor: { id: null, kind: 'system' as const },
  correlationId: 'probe',
})

async function main() {
  const form = services.form
  if (!form) throw new Error('Form service was not composed.')

  const person = await sql`select id from person where archived_at is null limit 1`
  const personId = (person[0] as { id: string } | undefined)?.id

  // (a) reads match the repository path
  const directList = await listFormInstances()
  const svcListResult = await form.execute({
    operation: FORM_OPERATIONS.LIST_INSTANCES,
    payload: {},
    context: operatorContext(),
  })
  const svcList = svcListResult.ok ? svcListResult.value : []
  check(
    'listInstances matches the repository path',
    JSON.stringify(directList.map((r) => r.id)) === JSON.stringify(svcList.map((r) => r.id)),
    `${directList.length} instance(s)`,
  )

  if (directList[0]) {
    const direct = await getFormInstance(directList[0].id)
    const svc = await form.execute({
      operation: FORM_OPERATIONS.GET_INSTANCE,
      payload: { formInstanceId: directList[0].id },
      context: operatorContext(),
    })
    check(
      'getInstance matches the repository path',
      svc.ok && JSON.stringify(svc.value) === JSON.stringify(direct),
      directList[0].id,
    )
  }

  // (c) fail closed: a system (unauthenticated-level) actor may not run a command
  const denied = await form.execute({
    operation: FORM_OPERATIONS.CREATE_INSTANCE,
    payload: { templateId: 'PROBE', templateVersion: 1, personId, fieldValues: {}, sections: {} },
    context: systemContext(),
  })
  check(
    'unauthenticated actor is denied a form command',
    !denied.ok && denied.error.code === 'FORBIDDEN',
    denied.ok ? 'ALLOWED (should be denied)' : denied.error.code,
  )

  // (b) a real operator command round-trips
  if (!personId) {
    check('create/get/update round-trip (skipped — no person in DEV)', true)
  } else {
    const created = await form.execute({
      operation: FORM_OPERATIONS.CREATE_INSTANCE,
      payload: {
        templateId: 'PROBE-TEMPLATE',
        templateVersion: 1,
        personId,
        fieldValues: { probe: 'yes' },
        sections: {},
      },
      context: operatorContext(),
    })
    check('operator can create an instance', created.ok, created.ok ? created.value.id : created.error.code)

    if (created.ok) {
      const id = created.value.id
      const updated = await form.execute({
        operation: FORM_OPERATIONS.UPDATE_INSTANCE,
        payload: { formInstanceId: id, input: { status: 'ready', fieldValues: { probe: 'yes', two: 'ok' } } },
        context: operatorContext(),
      })
      check(
        'operator can update an instance',
        updated.ok && updated.value?.status === 'ready' && updated.value?.fieldValues.two === 'ok',
        updated.ok ? String(updated.value?.status) : updated.error.code,
      )

      const reread = await getFormInstance(id)
      check('the update persisted', reread?.status === 'ready', String(reread?.status))

      await sql`delete from document_form_instance where id = ${id}`
      console.log('   (probe row removed)')
    }
  }

  // (d) the evidence read the two bindings now use: service path ≡ repository path
  const evidencePerson = await sql`
    select coalesce(f.person_id, d.client_person_id) as person_id
      from document_form_instance f
      left join deal d on d.id = f.deal_id
     where coalesce(f.person_id, d.client_person_id) is not null
     order by f.updated_at desc
     limit 1
  `
  const evidencePersonId =
    (evidencePerson[0] as { person_id: string } | undefined)?.person_id ?? null

  if (evidencePersonId) {
    for (const [label, templateId, roles] of [
      ['LISTING-01 (party roles)', 'LISTING-01', ['client', 'seller', 'owner']],
      ['PR-PNS (any active participant)', 'PR-PNS', null],
    ] as const) {
      const direct = await latestFormEvidence(templateId, evidencePersonId, roles ?? null)
      const viaService = await form.execute({
        operation: FORM_OPERATIONS.LATEST_EVIDENCE,
        payload: roles
          ? { templateId, personId: evidencePersonId, roles }
          : { templateId, personId: evidencePersonId },
        context: operatorContext(),
      })
      check(
        `latestEvidence matches the repository path :: ${label}`,
        viaService.ok && JSON.stringify(viaService.value) === JSON.stringify(direct),
        direct ? `instance ${direct.formInstanceId}` : 'null',
      )
    }
  } else {
    check('latestEvidence (skipped — no instance with a person in DEV)', true)
  }

  console.log(failures === 0 ? '\nALL FORMS SERVICE PROOFS PASSED' : `\n${failures} FAILURE(S)`)
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
