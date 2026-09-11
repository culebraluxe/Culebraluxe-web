import type {
  AttachPersonIdentityRequest,
  FindPersonByIdentityRequest,
  PersonDto,
  PersonIdentityDto,
  PersonSearchResult,
  SearchPeopleRequest,
  SetPersonDisplayNameRequest,
} from './types'

/** Persistence boundary shared by both kick-the-tires service styles. */
export interface PersonRepository {
  get(personId: string): Promise<PersonDto | null>
  findByIdentity(request: FindPersonByIdentityRequest): Promise<PersonDto | null>
  setDisplayName(request: SetPersonDisplayNameRequest): Promise<PersonDto>
  attachIdentity(request: AttachPersonIdentityRequest): Promise<PersonIdentityDto>
  search(request: SearchPeopleRequest): Promise<PersonSearchResult[]>
}
