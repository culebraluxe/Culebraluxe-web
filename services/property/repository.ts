import type {
  FindPropertyByAddressRequest,
  PersonPropertyContextDto,
  PropertyDto,
  SetPropertyDisplayNameRequest,
  SetPropertyStatusRequest,
} from './types'

export interface PropertyRepository {
  get(propertyId: string): Promise<PropertyDto | null>
  findByAddress(request: FindPropertyByAddressRequest): Promise<PropertyDto | null>
  forPerson(personId: string): Promise<PersonPropertyContextDto>
  setDisplayName(request: SetPropertyDisplayNameRequest): Promise<PropertyDto>
  setStatus(request: SetPropertyStatusRequest): Promise<PropertyDto>
}
