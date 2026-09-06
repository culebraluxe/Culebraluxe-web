import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

export type ContractParticipantDto = {
  personId: string
  role: string
}

export type ContractDto = {
  id: string
  contractType: string
  formTemplateId: string
  propertyId: string
  participants: readonly ContractParticipantDto[]
  facts: Readonly<Record<string, unknown>>
  status: string
  executedAt: string | null
}

export type ContractEffectiveStateDto = {
  contractId: string
  facts: Readonly<Record<string, unknown>>
  sourceContractIds: readonly string[]
}

export type GetContractRequest = { contractId: string }

export type CreateContractFromFormRequest = {
  contractId: string
  contractType: string
  formTemplateId: string
  propertyId: string
  participants: readonly ContractParticipantDto[]
  facts: Readonly<Record<string, unknown>>
}

export type GetContractEffectiveStateRequest = { contractId: string }

export type ExecuteContractRequest = {
  contractId: string
  evidenceDocumentId?: string
}

export const CONTRACT_OPERATIONS = {
  GET: 'contract.get',
  CREATE_FROM_FORM: 'contract.createFromForm',
  GET_EFFECTIVE_STATE: 'contract.getEffectiveState',
  EXECUTE: 'contract.execute',
} as const

export type ContractOperationMap = {
  'contract.get': { request: GetContractRequest; response: ContractDto | null }
  'contract.createFromForm': { request: CreateContractFromFormRequest; response: ContractDto }
  'contract.getEffectiveState': {
    request: GetContractEffectiveStateRequest
    response: ContractEffectiveStateDto | null
  }
  'contract.execute': { request: ExecuteContractRequest; response: ContractDto }
}

export type ContractOperationName = ServiceOperationName<ContractOperationMap>
export type ContractEnvelope<K extends ContractOperationName = ContractOperationName> =
  ServiceEnvelopeFor<ContractOperationMap, K>
