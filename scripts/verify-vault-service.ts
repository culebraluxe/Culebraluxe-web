// PROOF: the Vault domain owns issued documents through the service.
//   (a) reads match the repository path exactly
//   (b) an unauthenticated actor is DENIED a vault command (fail closed)
// Read-only against PROD by default (APP_ENV=production), DEV otherwise.

import { composeCoreServices } from '../services/composition'
import { AuthorizationService } from '../services/entitlement/authorization-service'
import { StaticAuthorizationPolicyProvider } from '../services/entitlement/authorization-service'
import { SqlVaultRepository } from '../db/vault-service-repository'
import { getTransactionDocument, listIssuedDocuments } from '../db/transaction-document'
import { getIssuedDocumentForFormInstance } from '../db/issued-document'
import { VAULT_OPERATIONS } from '../services/vault'
import { sql } from '../db/client'

const services = composeCoreServices(
  {
    vault: new SqlVaultRepository(),
    person: null as never,
    firm: null as never,
    property: null as never,
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
  const vault = services.vault
  if (!vault) throw new Error('Vault service was not composed.')

  // (a) the Cabinet list matches the repository path
  const direct = await listIssuedDocuments()
  const viaService = await vault.execute({
    operation: VAULT_OPERATIONS.LIST_ISSUED_DOCUMENTS,
    payload: {},
    context: operatorContext(),
  })
  const svcList = viaService.ok ? viaService.value : []
  check(
    'listIssuedDocuments matches the repository path',
    JSON.stringify(direct.map((row) => row.id)) === JSON.stringify(svcList.map((row) => row.id)),
    `${direct.length} issued document(s)`,
  )

  // getDocument matches
  const sample = direct[0]
  if (sample) {
    const directDoc = await getTransactionDocument(sample.id)
    const svcDoc = await vault.execute({
      operation: VAULT_OPERATIONS.GET_DOCUMENT,
      payload: { documentId: sample.id },
      context: operatorContext(),
    })
    check(
      'getDocument matches the repository path',
      svcDoc.ok && JSON.stringify(svcDoc.value) === JSON.stringify(directDoc),
      `${sample.documentTypeLabel ?? sample.id}`,
    )

    // the artifact bytes resolve through the service
    if (directDoc?.mediaId) {
      const bytes = await vault.execute({
        operation: VAULT_OPERATIONS.MEDIA_BYTES,
        payload: { mediaId: directDoc.mediaId },
        context: operatorContext(),
      })
      const size = bytes.ok ? bytes.value?.bytes.length ?? 0 : 0
      check('the issued artifact bytes resolve', bytes.ok && size > 0, `${size} bytes`)
    }
  }

  // issued-for-form-instance matches (the Forms editor/issuance read)
  const withForm = direct.find((row) => row.id)
  const formRow = (await sql`
    select form_instance_id from transaction_document
     where form_instance_id is not null and source = 'generated'
     order by created_at desc limit 1
  `) as Array<{ form_instance_id: string }>
  const formInstanceId = formRow[0]?.form_instance_id
  if (formInstanceId && withForm) {
    const directIssued = await getIssuedDocumentForFormInstance(formInstanceId)
    const svcIssued = await vault.execute({
      operation: VAULT_OPERATIONS.ISSUED_FOR_FORM_INSTANCE,
      payload: { formInstanceId },
      context: operatorContext(),
    })
    check(
      'issuedForFormInstance matches the repository path',
      svcIssued.ok && JSON.stringify(svcIssued.value) === JSON.stringify(directIssued),
      directIssued ? `v${directIssued.issuedVersion}` : 'null',
    )
  }

  // (b) fail closed: a system (unauthenticated-level) actor may not write
  const denied = await vault.execute({
    operation: VAULT_OPERATIONS.BIND_FORM_TO_CONTRACT,
    payload: { formInstanceId, contractId: '00000000-0000-4000-8000-000000000000' },
    context: systemContext(),
  })
  check(
    'unauthenticated actor is denied a vault command',
    !denied.ok && denied.error.code === 'FORBIDDEN',
    denied.ok ? 'ALLOWED (should be denied)' : denied.error.code,
  )

  console.log(failures === 0 ? '\nALL VAULT SERVICE PROOFS PASSED' : `\n${failures} FAILURE(S)`)
  process.exit(failures === 0 ? 0 : 1)
}

main().catch((error) => {
  console.error(error)
  process.exit(1)
})
