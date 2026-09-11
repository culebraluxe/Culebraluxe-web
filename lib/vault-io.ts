import 'server-only'

import { randomUUID } from 'node:crypto'

import { captureServerError } from '@/lib/server-error-capture'
import { coreServices } from '@/lib/service-runtime'
import { getActingUser } from '@/lib/auth/get-acting-user'
import { getPortalSessionAdapter } from '@/lib/auth/portal-session'
import { resolveSecurityLevel } from '@/services/security'
import { VAULT_OPERATIONS } from '@/services/vault'
import type {
  ContractIssuedLineage,
  IssuedDocumentForFormInstance,
  IssuedDocumentListItem,
  IssueDocumentInput,
  TransactionDocument,
  VaultMediaBytes,
} from '@/services/vault'
import type { ActingUser } from '@/lib/auth/types'
import type { CommandResult } from '@/lib/commands/contracts'

/**
 * Issued-document I/O for server surfaces — through the VAULT SERVICE, never the
 * database. (The retired direct calls reached db/transaction-document.ts and
 * db/issued-document.ts; those are now repository internals.)
 *
 * The call shape matches what the Cabinet, the download route, the Forms
 * issuance path and the form bindings already expect.
 */

export type {
  ContractIssuedLineage,
  IssuedDocumentForFormInstance,
  IssuedDocumentListItem,
  IssueDocumentInput,
  TransactionDocument,
  VaultMediaBytes,
}

function vaultService() {
  const service = coreServices.vault
  if (!service) throw new Error('Vault service is not composed in this runtime.')
  return service
}

/** The acting operator, so vault commands are authorized as that user. */
async function serviceContext() {
  const correlationId = randomUUID()
  try {
    const acting = await getActingUser(getPortalSessionAdapter())
    return {
      actor: { id: acting.appUserId, kind: 'user' as const },
      correlationId,
      principal: {
        appUserId: acting.appUserId,
        level: resolveSecurityLevel(acting.roleCodes),
        roleCodes: acting.roleCodes,
      },
    }
  } catch {
    // No session (background/route context): reads still work, commands fail closed.
    return { actor: { id: null, kind: 'system' as const }, correlationId }
  }
}

async function run<T>(operation: string, payload: unknown, label: string): Promise<T> {
  const result = await vaultService().execute({
    operation: operation as never,
    payload: payload as never,
    context: await serviceContext(),
  })
  if (!result.ok) {
    captureServerError(label, new Error(`${result.error.code}: ${result.error.message}`), {
      level: 'error',
    })
    throw new Error(result.error.message)
  }
  return result.value as T
}

/* ------------------------------------------------------------------ *
 * Reads
 * ------------------------------------------------------------------ */

/** Issued documents, newest first. External actors see only their own deals. */
export async function listIssuedDocuments(
  actor?: Pick<ActingUser, 'accountType' | 'personId'>,
): Promise<IssuedDocumentListItem[]> {
  return run<IssuedDocumentListItem[]>(
    VAULT_OPERATIONS.LIST_ISSUED_DOCUMENTS,
    actor ? { actor } : {},
    'vault:listIssuedDocuments',
  )
}

export async function getTransactionDocument(id: string): Promise<TransactionDocument | null> {
  return run<TransactionDocument | null>(
    VAULT_OPERATIONS.GET_DOCUMENT,
    { documentId: id },
    'vault:getDocument',
  )
}

export async function listTransactionDocumentsByDeal(dealId: string): Promise<TransactionDocument[]> {
  return run<TransactionDocument[]>(
    VAULT_OPERATIONS.LIST_BY_DEAL,
    { dealId },
    'vault:listByDeal',
  )
}

/** The exact stored bytes of an issued artifact (download / verification). */
export async function getMediaBytes(mediaId: string): Promise<VaultMediaBytes | null> {
  return run<VaultMediaBytes | null>(
    VAULT_OPERATIONS.MEDIA_BYTES,
    { mediaId },
    'vault:mediaBytes',
  )
}

/** The issued document a form instance produced, when any. */
export async function getIssuedDocumentForFormInstance(
  formInstanceId: string,
): Promise<IssuedDocumentForFormInstance | null> {
  return run<IssuedDocumentForFormInstance | null>(
    VAULT_OPERATIONS.ISSUED_FOR_FORM_INSTANCE,
    { formInstanceId },
    'vault:issuedForFormInstance',
  )
}

/** The Contract a form instance is bound to, when any. */
export async function getFormContractId(formInstanceId: string): Promise<string | null> {
  return run<string | null>(
    VAULT_OPERATIONS.FORM_CONTRACT_ID,
    { formInstanceId },
    'vault:formContractId',
  )
}

/** The prior issued document for a Contract + template (lineage/supersession). */
export async function getPriorContractIssuedDocument(input: {
  contractId: string
  templateId: string
}): Promise<ContractIssuedLineage | null> {
  return run<ContractIssuedLineage | null>(
    VAULT_OPERATIONS.PRIOR_CONTRACT_DOCUMENT,
    input,
    'vault:priorContractDocument',
  )
}

/* ------------------------------------------------------------------ *
 * Commands
 * ------------------------------------------------------------------ */

/** Issue a form instance as immutable evidence (one transaction). */
export async function issueFormDocument(input: IssueDocumentInput): Promise<CommandResult> {
  return run<CommandResult>(
    VAULT_OPERATIONS.ISSUE_FROM_FORM_INSTANCE,
    input,
    'vault:issueFromFormInstance',
  )
}

/** Bind a form instance to its Contract (never transferable). */
export async function bindFormInstanceToContract(input: {
  formInstanceId: string
  contractId: string
}): Promise<void> {
  await run<void>(VAULT_OPERATIONS.BIND_FORM_TO_CONTRACT, input, 'vault:bindFormToContract')
}
