import type {
  FindPropertyByAddressRequest,
  PersonPropertyContextDto,
  PropertyDto,
  PropertyForPersonDto,
  SetPropertyDisplayNameRequest,
  SetPropertyStatusRequest,
  UpsertPropertyForPersonRequest,
} from './types'

export interface PropertyRepository {
  get(propertyId: string): Promise<PropertyDto | null>
  findByAddress(request: FindPropertyByAddressRequest): Promise<PropertyDto | null>
  forPerson(personId: string): Promise<PersonPropertyContextDto>
  upsertForPerson(request: UpsertPropertyForPersonRequest): Promise<PropertyForPersonDto>
  setDisplayName(request: SetPropertyDisplayNameRequest): Promise<PropertyDto>
  setStatus(request: SetPropertyStatusRequest): Promise<PropertyDto>
}
