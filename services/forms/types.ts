import type { ServiceEnvelopeFor, ServiceOperationName } from '../core'
import type { FormSignerPerson } from '@/lib/forms/signer-resolution'

/** Re-exported so the domain contract is the single import for signer people. */
export type { FormSignerPerson }

/* ------------------------------------------------------------------ *
 * FORMS — the document form instance domain.
 *
 * A form instance is the MUTABLE working state: the editable assembly of a
 * TemplateDefinition against a deal/client/property. It is NEVER the immutable
 * business record — issuance snapshots these values into a new
 * transaction_document row and marks the instance 'issued'.
 *
 * The form BINDINGS (which fields come from which domain) already go through the
 * kernel (contract/showing/property/person/firm). This domain owns the instance
 * itself: its lifecycle, its participants, and the canonical facts used to
 * prefill it.
 * ------------------------------------------------------------------ */

export type FormInstanceStatus = 'draft' | 'ready' | 'issued'

export type FormInstance = {
  id: string
  templateId: string
  templateVersion: number
  dealId: string | null
  personId: string | null
  propertyId: string | null
  /**
   * The Contract this form produced (the artifact link). The form is the INPUT;
   * the contract is what it creates. Written back after the bridge runs so a
   * second save updates the same contract instead of creating another one.
   */
  contractId: string | null
  status: FormInstanceStatus
  fieldValues: Record<string, string>
  sections: Record<string, string>
  createdByUserId: string | null
  createdAt: string
  updatedAt: string
}

/** Form instance with deal/property/client labels for the Forms listing. */
export type FormInstanceListItem = FormInstance & {
  dealLabel: string | null
  propertyLabel: string | null
  clientName: string | null
}

/** Canonical deal facts available for form prefill (DOC-07). */
export type DealFormFacts = {
  clientName: string | null
  propertyLabel: string | null
  offerAmount: string | null
  financingType: string | null
  closingDate: string | null
  personDisplayName?: string | null
  propertyName?: string | null
  propertyLocation?: string | null
}

export type CreateFormInstanceInput = {
  templateId: string
  templateVersion: number
  dealId?: string | null
  personId?: string | null
  propertyId?: string | null
  fieldValues: Record<string, string>
  sections: Record<string, string>
  createdByUserId?: string | null
}

export type UpdateFormInstanceInput = {
  fieldValues?: Record<string, string>
  sections?: Record<string, string>
  status?: FormInstanceStatus
  /** Set once the form has produced a Contract (the artifact link). */
  contractId?: string | null
}

export type CreateFormInstanceRequest = CreateFormInstanceInput
export type GetFormInstanceRequest = { formInstanceId: string }
export type UpdateFormInstanceRequest = { formInstanceId: string; input: UpdateFormInstanceInput }
export type ListFormInstancesRequest = Record<string, never>
export type GetDealFormFactsRequest = { dealId: string }
export type SeedFormParticipantsRequest = { formInstanceId: string; dealId: string }

/**
 * The latest form instance of one template for a person — the "form evidence"
 * a binding reads when a canonical field is empty. A person matches either as the
 * instance's person, the deal's client, or an active deal participant.
 *
 * `roles` narrows which participant roles count (omitted = any active
 * participant), so each caller keeps its own evidence semantics.
 */
export type LatestFormEvidenceRequest = {
  templateId: string
  personId: string
  roles?: readonly string[]
}

export type FormInstanceEvidence = {
  formInstanceId: string
  propertyId: string | null
  fieldValues: Record<string, string>
  updatedAt: string | null
}

/* ------------------------------------------------------------------ *
 * LINEAGE — what a form instance is bound to (deal / client / property /
 * showing / contract). The instance is the mutable draft; issued documents are
 * never rebound.
 * ------------------------------------------------------------------ */

export type DirectFormContext = {
  personId: string
  propertyId: string
}

export type ResolveDealLaunchContextRequest = { dealId: string }
export type BindFormInstanceToDirectContextRequest = {
  formInstanceId: string
  personId: string
  propertyId: string
}
export type BindListingFormContextRequest = {
  formInstanceId: string
  personId: string
  propertyId: string | null
}
export type GetFormShowingIdRequest = { formInstanceId: string }
export type BindFormInstanceToShowingRequest = { formInstanceId: string; showingId: string }

/* ------------------------------------------------------------------ *
 * SIGNERS — who signs this instance. Resolved from the instance's own person
 * plus its deal participants, and (for Listing / Purchase & Sale) the
 * configured CulebraLuxe broker.
 * ------------------------------------------------------------------ */

export type ListFormSignerPeopleRequest = { formInstanceId: string }

export const FORM_OPERATIONS = {
  CREATE_INSTANCE: 'form.createInstance',
  GET_INSTANCE: 'form.getInstance',
  UPDATE_INSTANCE: 'form.updateInstance',
  LIST_INSTANCES: 'form.listInstances',
  DEAL_FACTS: 'form.dealFacts',
  SEED_PARTICIPANTS: 'form.seedParticipantsFromDeal',
  LATEST_EVIDENCE: 'form.latestEvidence',
  RESOLVE_DEAL_LAUNCH_CONTEXT: 'form.resolveDealLaunchContext',
  BIND_DIRECT_CONTEXT: 'form.bindDirectContext',
  BIND_LISTING_CONTEXT: 'form.bindListingContext',
  GET_SHOWING_ID: 'form.getShowingId',
  BIND_SHOWING: 'form.bindShowing',
  LIST_SIGNER_PEOPLE: 'form.listSignerPeople',
} as const

export type FormOperationMap = {
  'form.createInstance': { request: CreateFormInstanceRequest; response: FormInstance }
  'form.getInstance': { request: GetFormInstanceRequest; response: FormInstance | null }
  'form.updateInstance': { request: UpdateFormInstanceRequest; response: FormInstance | null }
  'form.listInstances': { request: ListFormInstancesRequest; response: FormInstanceListItem[] }
  'form.dealFacts': { request: GetDealFormFactsRequest; response: DealFormFacts | null }
  'form.seedParticipantsFromDeal': { request: SeedFormParticipantsRequest; response: void }
  'form.latestEvidence': { request: LatestFormEvidenceRequest; response: FormInstanceEvidence | null }
  'form.resolveDealLaunchContext': {
    request: ResolveDealLaunchContextRequest
    response: DirectFormContext | null
  }
  'form.bindDirectContext': {
    request: BindFormInstanceToDirectContextRequest
    response: void
  }
  'form.bindListingContext': {
    request: BindListingFormContextRequest
    response: void
  }
  'form.getShowingId': { request: GetFormShowingIdRequest; response: string | null }
  'form.bindShowing': { request: BindFormInstanceToShowingRequest; response: void }
  'form.listSignerPeople': { request: ListFormSignerPeopleRequest; response: FormSignerPerson[] }
}

export type FormOperationName = ServiceOperationName<FormOperationMap>
export type FormEnvelope<K extends FormOperationName = FormOperationName> =
  ServiceEnvelopeFor<FormOperationMap, K>
