import type {
  FindPropertyByAddressRequest,
  PropertyDto,
  SetPropertyDisplayNameRequest,
  SetPropertyStatusRequest,
} from './types'

export interface PropertyRepository {
  get(propertyId: string): Promise<PropertyDto | null>
  findByAddress(request: FindPropertyByAddressRequest): Promise<PropertyDto | null>
  setDisplayName(request: SetPropertyDisplayNameRequest): Promise<PropertyDto>
  setStatus(request: SetPropertyStatusRequest): Promise<PropertyDto>
}
