import type { QueryExecutor } from './query-executor'
import {
  createTransactionDocument,
  getTransactionDocument,
  listIssuedDocuments,
  listTransactionDocumentsByDeal,
  transitionTransactionDocumentState,
} from './transaction-document'
import {
  getIssuedDocumentForFormInstance,
  getMediaBytes,
  getNextIssuedVersionForTemplate,
  issueFormDocument,
} from './issued-document'
import {
  bindFormInstanceToContract,
  getFormContractId,
  getPriorContractIssuedDocument,
} from './contract-issued-document'
import type { CommandResult } from '@/lib/commands/contracts'
import type { VaultRepository } from '@/services/vault'

/**
 * Vault persistence boundary. The SQL lives in the repository-internal modules
 * (transaction-document / issued-document / contract-issued-document); this
 * class is the only thing that reaches them.
 *
 * Exception: the transactional `document.issue` command adapter calls
 * db/issued-document.ts directly because its body must share the COMMAND's
 * transaction (ctx.run). Request/response surfaces go through VaultService.
 */
export class SqlVaultRepository implements VaultRepository {
  constructor(private readonly execute?: QueryExecutor) {}

  listIssuedDocuments(request: Parameters<VaultRepository['listIssuedDocuments']>[0]) {
    return listIssuedDocuments(this.execute, request.actor)
  }

  getDocument(request: { documentId: string }) {
    return getTransactionDocument(request.documentId, this.execute)
  }

  listByDeal(request: { dealId: string }) {
    return listTransactionDocumentsByDeal(request.dealId, this.execute)
  }

  createDocument(request: Parameters<VaultRepository['createDocument']>[0]) {
    return createTransactionDocument(request, this.execute)
  }

  transitionState(request: {
    documentId: string
    input: Parameters<VaultRepository['transitionState']>[0]['input']
  }): Promise<CommandResult> {
    return transitionTransactionDocumentState(request.documentId, request.input)
  }

  issueFromFormInstance(
    request: Parameters<VaultRepository['issueFromFormInstance']>[0],
  ): Promise<CommandResult> {
    return issueFormDocument(request)
  }

  issuedForFormInstance(request: { formInstanceId: string }) {
    return getIssuedDocumentForFormInstance(request.formInstanceId, this.execute)
  }

  nextIssuedVersion(request: {
    contractId?: string | null
    dealId?: string | null
    templateId: string
  }) {
    return getNextIssuedVersionForTemplate(request, this.execute)
  }

  mediaBytes(request: { mediaId: string }) {
    return getMediaBytes(request.mediaId, this.execute)
  }

  formContractId(request: { formInstanceId: string }) {
    return getFormContractId(request.formInstanceId, this.execute)
  }

  bindFormToContract(request: { formInstanceId: string; contractId: string }) {
    return bindFormInstanceToContract(request, this.execute)
  }

  priorContractDocument(request: { contractId: string; templateId: string }) {
    return getPriorContractIssuedDocument(request, this.execute)
  }
}
