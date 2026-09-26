import 'server-only'

import { captureServerError } from '@/lib/server-error-capture'
import {
  RustApiError,
  rustApiBindVaultFormContract,
  rustApiVaultDocument,
  rustApiVaultDocumentBytes,
  rustApiVaultDocuments,
  rustApiVaultDocumentsByDeal,
  rustApiVaultFormContract,
  rustApiVaultIssuedDocumentForForm,
  rustApiVaultPriorContractDocument,
} from '@/lib/rust-api/client'
import type { ActingUser } from '@/lib/auth/types'
import type { CommandResult } from '@/lib/workflow/contracts'
import {
  mapRustVaultTransactionDocument,
  type ContractIssuedLineage,
  type IssuedDocumentForFormInstance,
  type IssuedDocumentListItem,
  type IssueDocumentInput,
  type RustVaultTransactionDocument,
  type TransactionDocument,
  type VaultMediaBytes,
} from '@/lib/vault-contract'

export type {
  ContractIssuedLineage,
  IssuedDocumentForFormInstance,
  IssuedDocumentListItem,
  IssueDocumentInput,
  TransactionDocument,
  VaultMediaBytes,
} from '@/lib/vault-contract'

/**
 * Issued-document I/O for server surfaces.
 *
 * Persistence and authorization are Rust-owned. The one temporary compatibility
 * exception is PDF issuance: its artifact renderer/broker-signature seam still
 * lives with the Forms migration and is intentionally isolated below until that
 * coordinated cutover lands.
 */

/**
 * Preserve the historical call shape without trusting a TS-supplied actor.
 * Rust resolves the authenticated principal and applies Vault scoping itself.
 */
export async function listIssuedDocuments(
  _actor?: Pick<ActingUser, 'accountType' | 'personId'>,
): Promise<IssuedDocumentListItem[]> {
  const value = await rustApiVaultDocuments<Array<IssuedDocumentListItem & { dealId: string | null }>>()
  return value.map((item) => ({ ...item, dealId: item.dealId ?? '' }))
}

export async function getTransactionDocument(id: string): Promise<TransactionDocument | null> {
  try {
    const value = await rustApiVaultDocument<RustVaultTransactionDocument>(id)
    return mapRustVaultTransactionDocument(value)
  } catch (error) {
    if (error instanceof RustApiError && error.status === 404) return null
    throw error
  }
}

export async function listTransactionDocumentsByDeal(
  dealId: string,
): Promise<TransactionDocument[]> {
  const values =
    await rustApiVaultDocumentsByDeal<RustVaultTransactionDocument[]>(dealId)
  return values.map(mapRustVaultTransactionDocument)
}

export async function getMediaBytes(mediaId: string): Promise<VaultMediaBytes | null> {
  return rustApiVaultDocumentBytes(mediaId)
}

export async function getIssuedDocumentForFormInstance(
  formInstanceId: string,
): Promise<IssuedDocumentForFormInstance | null> {
  return rustApiVaultIssuedDocumentForForm<IssuedDocumentForFormInstance | null>(
    formInstanceId,
  )
}

export async function getFormContractId(formInstanceId: string): Promise<string | null> {
  return rustApiVaultFormContract<string | null>(formInstanceId)
}

export async function getPriorContractIssuedDocument(input: {
  contractId: string
  templateId: string
}): Promise<ContractIssuedLineage | null> {
  return rustApiVaultPriorContractDocument<ContractIssuedLineage | null>(
    input.contractId,
    input.templateId,
  )
}

/**
 * Temporary coordinated-Forms seam.
 *
 * Do not move persistence back into TS: the legacy function is retained only
 * because the current Rust HTTP Vault transport has no artifact renderer yet.
 * Deep's Forms/signature landing should replace this function with the Rust
 * issuance command, after which vault-io has zero legacy reachability.
 */
export async function issueFormDocument(input: IssueDocumentInput): Promise<CommandResult> {
  try {
    const legacy = await import('@/legacy/db/issued-document')
    return await legacy.issueFormDocument(input)
  } catch (error) {
    captureServerError('vault:issueFromFormInstance', error, { level: 'error' })
    throw error
  }
}

export async function bindFormInstanceToContract(input: {
  formInstanceId: string
  contractId: string
}): Promise<void> {
  await rustApiBindVaultFormContract(input.formInstanceId, input.contractId)
}
