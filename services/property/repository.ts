import type {
  FilterPropertiesRequest,
  FindPropertyByAddressRequest,
  GetPropertyBySlugRequest,
  GetPropertyIntroRequest,
  GetSimilarPropertiesRequest,
  ListPropertiesRequest,
  PersonPropertyContextDto,
  PropertyDto,
  PropertyForPersonDto,
  PropertyIntro,
  PropertyInventoryPage,
  PropertySummary,
  SetPropertyDisplayNameRequest,
  SetPropertyStatusRequest,
  UpsertPropertyForPersonRequest,
} from './types'
import type { Result } from '@/db/client'
import type { PropertyDetailResult } from '@/lib/property-types'

export interface PropertyRepository {
  get(propertyId: string): Promise<PropertyDto | null>
  findByAddress(request: FindPropertyByAddressRequest): Promise<PropertyDto | null>
  forPerson(personId: string): Promise<PersonPropertyContextDto>
  upsertForPerson(request: UpsertPropertyForPersonRequest): Promise<PropertyForPersonDto>
  setDisplayName(request: SetPropertyDisplayNameRequest): Promise<PropertyDto>
  setStatus(request: SetPropertyStatusRequest): Promise<PropertyDto>

  // Public inventory reads (active listings). They return the gateway Result so a
  // failed read degrades a page instead of rejecting it (DB-HARDEN-01C).
  list(request: ListPropertiesRequest): Promise<Result<PropertySummary[]>>
  search(request: FilterPropertiesRequest): Promise<Result<PropertyInventoryPage>>
  similar(request: GetSimilarPropertiesRequest): Promise<Result<PropertySummary[]>>
  bySlug(request: GetPropertyBySlugRequest): Promise<Result<PropertyDetailResult | null>>
  publicSlugs(): Promise<Result<string[]>>
  intro(request: GetPropertyIntroRequest): Promise<Result<PropertyIntro | null>>
}
