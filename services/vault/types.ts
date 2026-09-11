import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'
import type { CommandResult } from '@/lib/commands/contracts'
import type { ActingUser } from '@/lib/auth/types'
import type {
  CreateTransactionDocumentInput,
  IssuedDocumentListItem,
  TransactionDocument,
  TransitionTransactionDocumentInput,
} from '@/db/transaction-document'
import type { IssueDocumentInput } from '@/db/issued-document'
import type { ContractIssuedLineage } from '@/db/contract-issued-document'

/* ------------------------------------------------------------------ *
 * VAULT — the issued-document domain (sister service to Forms).
 *
 * A form instance is mutable working state. When it is ISSUED, the Vault takes
 * the immutable record: the rendered artifact (media) and the immutable
 * transaction_document row that cannot be edited, only superseded.
 *
 * The Cabinet (/portal/documents) reads issued documents through here.
 *
 * DTO definitions currently live in the repository modules (db/transaction-
 * document.ts, db/issued-document.ts) and are re-exported type-only so there is
 * one shape, not two.
 * ------------------------------------------------------------------ */

export type {
  CreateTransactionDocumentInput,
  IssuedDocumentListItem,
  IssueDocumentInput,
  TransactionDocument,
  TransitionTransactionDocumentInput,
  ContractIssuedLineage,
}

export type ListIssuedDocumentsRequest = {
  /** External actors only see documents for deals their own person is in. */
  actor?: Pick<ActingUser, 'accountType' | 'personId'>
}
export type GetVaultDocumentRequest = { documentId: string }
export type ListDocumentsByDealRequest = { dealId: string }
export type IssuedForFormInstanceRequest = { formInstanceId: string }
export type NextIssuedVersionRequest = {
  contractId?: string | null
  dealId?: string | null
  templateId: string
}
export type VaultMediaBytesRequest = { mediaId: string }
export type FormContractIdRequest = { formInstanceId: string }
export type BindFormToContractRequest = { formInstanceId: string; contractId: string }
export type PriorContractDocumentRequest = { contractId: string; templateId: string }

export type IssuedDocumentForFormInstance = {
  documentId: string
  issuedVersion: number
  checksum: string
  createdAt: string
  mediaId: string | null
  sourceSnapshot: Record<string, unknown> | null
}

export type VaultMediaBytes = {
  bytes: Buffer
  filename: string
  mimeType: string
}

export const VAULT_OPERATIONS = {
  LIST_ISSUED_DOCUMENTS: 'vault.listIssuedDocuments',
  GET_DOCUMENT: 'vault.getDocument',
  LIST_BY_DEAL: 'vault.listByDeal',
  CREATE_DOCUMENT: 'vault.createDocument',
  TRANSITION_STATE: 'vault.transitionState',
  ISSUE_FROM_FORM_INSTANCE: 'vault.issueFromFormInstance',
  ISSUED_FOR_FORM_INSTANCE: 'vault.issuedForFormInstance',
  NEXT_ISSUED_VERSION: 'vault.nextIssuedVersion',
  MEDIA_BYTES: 'vault.mediaBytes',
  FORM_CONTRACT_ID: 'vault.formContractId',
  BIND_FORM_TO_CONTRACT: 'vault.bindFormToContract',
  PRIOR_CONTRACT_DOCUMENT: 'vault.priorContractDocument',
} as const

export type VaultOperationMap = {
  'vault.listIssuedDocuments': {
    request: ListIssuedDocumentsRequest
    response: IssuedDocumentListItem[]
  }
  'vault.getDocument': { request: GetVaultDocumentRequest; response: TransactionDocument | null }
  'vault.listByDeal': { request: ListDocumentsByDealRequest; response: TransactionDocument[] }
  'vault.createDocument': {
    request: CreateTransactionDocumentInput
    response: TransactionDocument
  }
  'vault.transitionState': {
    request: { documentId: string; input: TransitionTransactionDocumentInput }
    response: CommandResult
  }
  'vault.issueFromFormInstance': { request: IssueDocumentInput; response: CommandResult }
  'vault.issuedForFormInstance': {
    request: IssuedForFormInstanceRequest
    response: IssuedDocumentForFormInstance | null
  }
  'vault.nextIssuedVersion': { request: NextIssuedVersionRequest; response: number }
  'vault.mediaBytes': { request: VaultMediaBytesRequest; response: VaultMediaBytes | null }
  'vault.formContractId': { request: FormContractIdRequest; response: string | null }
  'vault.bindFormToContract': { request: BindFormToContractRequest; response: void }
  'vault.priorContractDocument': {
    request: PriorContractDocumentRequest
    response: ContractIssuedLineage | null
  }
}

export type VaultOperationName = ServiceOperationName<VaultOperationMap>
export type VaultEnvelope<K extends VaultOperationName = VaultOperationName> =
  ServiceEnvelopeFor<VaultOperationMap, K>
