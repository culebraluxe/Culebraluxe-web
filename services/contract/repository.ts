import type {
  ContractDto,
  ContractEffectiveStateDto,
  ContractSummaryDto,
  CreateContractFromFormRequest,
  SaveContractDraftRequest,
  ExecuteContractRequest,
} from './types'

export interface ContractRepository {
  get(contractId: string): Promise<ContractDto | null>
  list(): Promise<ContractSummaryDto[]>
  createFromForm(request: CreateContractFromFormRequest): Promise<ContractDto>
  saveDraft(request: SaveContractDraftRequest): Promise<ContractDto>
  getEffectiveState(contractId: string): Promise<ContractEffectiveStateDto | null>
  execute(request: ExecuteContractRequest): Promise<ContractDto>
}
