import type {
  FindFirmByNameRequest,
  FirmDto,
  UpsertFirmRequest,
} from '@/legacy/services/firm/types'

export interface FirmRepository {
  get(firmId: string): Promise<FirmDto | null>
  findByName(request: FindFirmByNameRequest): Promise<FirmDto | null>
  upsert(request: UpsertFirmRequest): Promise<FirmDto>
}
