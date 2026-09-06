import type {
  ContractDto,
  ContractEffectiveStateDto,
  CreateContractFromFormRequest,
  ExecuteContractRequest,
} from './types'

export interface ContractRepository {
  get(contractId: string): Promise<ContractDto | null>
  createFromForm(request: CreateContractFromFormRequest): Promise<ContractDto>
  getEffectiveState(contractId: string): Promise<ContractEffectiveStateDto | null>
  execute(request: ExecuteContractRequest): Promise<ContractDto>
}
