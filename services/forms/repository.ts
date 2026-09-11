import type {
  BindFormInstanceToDirectContextRequest,
  BindFormInstanceToShowingRequest,
  BindListingFormContextRequest,
  CreateFormInstanceRequest,
  DealFormFacts,
  DirectFormContext,
  FormInstance,
  FormInstanceEvidence,
  FormInstanceListItem,
  FormSignerPerson,
  GetDealFormFactsRequest,
  GetFormInstanceRequest,
  GetFormShowingIdRequest,
  LatestFormEvidenceRequest,
  ListFormSignerPeopleRequest,
  ResolveDealLaunchContextRequest,
  SeedFormParticipantsRequest,
  UpdateFormInstanceRequest,
} from './types'

/**
 * Persistence boundary for the form instance domain. The Forms service is the
 * only thing that reaches it; nothing outside a repository queries
 * document_form_instance / document_form_participant.
 */
export interface FormRepository {
  createInstance(request: CreateFormInstanceRequest): Promise<FormInstance>
  getInstance(request: GetFormInstanceRequest): Promise<FormInstance | null>
  updateInstance(request: UpdateFormInstanceRequest): Promise<FormInstance | null>
  listInstances(): Promise<FormInstanceListItem[]>
  dealFacts(request: GetDealFormFactsRequest): Promise<DealFormFacts | null>
  seedParticipantsFromDeal(request: SeedFormParticipantsRequest): Promise<void>
  latestEvidence(request: LatestFormEvidenceRequest): Promise<FormInstanceEvidence | null>
  resolveDealLaunchContext(request: ResolveDealLaunchContextRequest): Promise<DirectFormContext | null>
  bindDirectContext(request: BindFormInstanceToDirectContextRequest): Promise<void>
  bindListingContext(request: BindListingFormContextRequest): Promise<void>
  getShowingId(request: GetFormShowingIdRequest): Promise<string | null>
  bindShowing(request: BindFormInstanceToShowingRequest): Promise<void>
  listSignerPeople(request: ListFormSignerPeopleRequest): Promise<FormSignerPerson[]>
}
