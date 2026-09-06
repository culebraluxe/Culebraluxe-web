import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'

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
}

export type ContractFirmRoleDto = {
  kind: 'firm'
  firmId: string
  roleCode: string
  ordinal?: number
  snapshotName?: string | null
}

export type ContractRoleDto = ContractPersonRoleDto | ContractFirmRoleDto

export type ContractDto = {
  id: string
  contractType: string
  formTemplateId: string
  propertyId: string
  roles: readonly ContractRoleDto[]
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
  roles: readonly ContractRoleDto[]
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
