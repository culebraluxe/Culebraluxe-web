import type {
  BindFormToContractRequest,
  CreateTransactionDocumentInput,
  ContractIssuedLineage,
  GetVaultDocumentRequest,
  IssuedDocumentForFormInstance,
  IssuedDocumentListItem,
  IssueDocumentInput,
  IssuedForFormInstanceRequest,
  ListDocumentsByDealRequest,
  ListIssuedDocumentsRequest,
  NextIssuedVersionRequest,
  PriorContractDocumentRequest,
  TransactionDocument,
  TransitionTransactionDocumentInput,
  VaultMediaBytes,
  VaultMediaBytesRequest,
  FormContractIdRequest,
} from './types'
import type { CommandResult } from '@/lib/commands/contracts'

/**
 * Persistence boundary for the Vault (issued documents). The Vault service is
 * the only thing that reaches it; nothing outside a repository queries
 * transaction_document.
 */
export interface VaultRepository {
  listIssuedDocuments(request: ListIssuedDocumentsRequest): Promise<IssuedDocumentListItem[]>
  getDocument(request: GetVaultDocumentRequest): Promise<TransactionDocument | null>
  listByDeal(request: ListDocumentsByDealRequest): Promise<TransactionDocument[]>
  createDocument(request: CreateTransactionDocumentInput): Promise<TransactionDocument>
  transitionState(request: {
    documentId: string
    input: TransitionTransactionDocumentInput
  }): Promise<CommandResult>
  issueFromFormInstance(request: IssueDocumentInput): Promise<CommandResult>
  issuedForFormInstance(
    request: IssuedForFormInstanceRequest,
  ): Promise<IssuedDocumentForFormInstance | null>
  nextIssuedVersion(request: NextIssuedVersionRequest): Promise<number>
  mediaBytes(request: VaultMediaBytesRequest): Promise<VaultMediaBytes | null>
  formContractId(request: FormContractIdRequest): Promise<string | null>
  bindFormToContract(request: BindFormToContractRequest): Promise<void>
  priorContractDocument(
    request: PriorContractDocumentRequest,
  ): Promise<ContractIssuedLineage | null>
}
