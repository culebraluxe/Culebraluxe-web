import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

export type ContractRoleAttributes = Readonly<Record<string, unknown>>

/**
 * A Role is the contextual position an identity occupies in this Contract.
 * `kind` is the TypeScript discriminator: Person and Firm stay strongly typed
 * while the Contract gets one simple Role collection.
 */
export type ContractPersonRoleDto = {
  kind: 'person'
  personId: string
  roleCode: string
  ordinal?: number
  snapshotName?: string | null
  attributes?: ContractRoleAttributes
}

export type ContractFirmRoleDto = {
  kind: 'firm'
  firmId: string
  roleCode: string
  ordinal?: number
  snapshotName?: string | null
  attributes?: ContractRoleAttributes
}

export type ContractRoleDto = ContractPersonRoleDto | ContractFirmRoleDto

export type ContractDto = {
  id: string
  contractType: string
  formTemplateId: string
  sourceFormInstanceId: string | null
  predecessorContractId: string | null
  /**
   * The cord: the workflow/process instance (transaction) this Contract belongs
   * to. Null when the Contract stands alone (a showing report that goes nowhere).
   * See docs/REAL-ESTATE-TRANSACTION-DESIGN.md section 5.1.
   */
  processInstanceId: string | null
  propertyId: string
  roles: readonly ContractRoleDto[]
  facts: Readonly<Record<string, unknown>>
  status: string
  executedAt: string | null
  evidenceDocumentId: string | null
}

export type ContractEffectiveStateDto = {
  contractId: string
  facts: Readonly<Record<string, unknown>>
  sourceContractIds: readonly string[]
}

export type GetContractRequest = { contractId: string }

/**
 * Portfolio row for the Contracts surface: one contract, its place in the
 * workflow chain, and the Vault document that evidences it.
 * The chain IS the workflow: Listing -> P&S -> Closing (seller),
 * Showing Report -> Offer -> P&S -> Closing (buyer).
 */
export type ContractSummaryDto = {
  id: string
  contractType: string
  formTemplateId: string
  status: string
  propertyId: string
  predecessorContractId: string | null
  /** The cord — see ContractDto.processInstanceId. */
  processInstanceId: string | null
  evidenceDocumentId: string | null
  executedAt: string | null
  createdAt: string
}
export type ListContractsRequest = Record<string, never>

/** The cord, read the other way: every Contract attached to one transaction. */
export type ListContractsForProcessInstanceRequest = { processInstanceId: string }

export type CreateContractFromFormRequest = {
  contractId: string
  contractType: string
  formTemplateId: string
  sourceFormInstanceId?: string | null
  predecessorContractId?: string | null
  /** Attach the Contract to its transaction (the cord). Optional by design. */
  processInstanceId?: string | null
  propertyId: string
  roles: readonly ContractRoleDto[]
  facts: Readonly<Record<string, unknown>>
}

/**
 * Mutable working-state save used by Forms before a Contract is issued/executed.
 * Same semantic shape as creation: the complete draft replaces the prior draft
 * projection. Executed/non-draft Contracts are immutable through this command.
 */
export type SaveContractDraftRequest = CreateContractFromFormRequest

export type GetContractEffectiveStateRequest = { contractId: string }

export type ExecuteContractRequest = {
  contractId: string
  evidenceDocumentId?: string
}

export const CONTRACT_OPERATIONS = {
  GET: 'contract.get',
  LIST: 'contract.list',
  LIST_FOR_PROCESS_INSTANCE: 'contract.listForProcessInstance',
  CREATE_FROM_FORM: 'contract.createFromForm',
  SAVE_DRAFT: 'contract.saveDraft',
  GET_EFFECTIVE_STATE: 'contract.getEffectiveState',
  EXECUTE: 'contract.execute',
} as const

export type ContractOperationMap = {
  'contract.get': { request: GetContractRequest; response: ContractDto | null }
  'contract.list': { request: ListContractsRequest; response: ContractSummaryDto[] }
  'contract.listForProcessInstance': {
    request: ListContractsForProcessInstanceRequest
    response: ContractSummaryDto[]
  }
  'contract.createFromForm': { request: CreateContractFromFormRequest; response: ContractDto }
  'contract.saveDraft': { request: SaveContractDraftRequest; response: ContractDto }
  'contract.getEffectiveState': {
    request: GetContractEffectiveStateRequest
    response: ContractEffectiveStateDto | null
  }
  'contract.execute': { request: ExecuteContractRequest; response: ContractDto }
}

export type ContractOperationName = ServiceOperationName<ContractOperationMap>
export type ContractEnvelope<K extends ContractOperationName = ContractOperationName> =
  ServiceEnvelopeFor<ContractOperationMap, K>
