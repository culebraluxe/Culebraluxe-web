import 'server-only'

import type { FormInstance } from '@/db/document-form-instance'
import { getFormContractId } from '@/db/contract-issued-document'
import { getFormShowingId } from '@/db/form-service-lineage'
import { CONTRACT_OPERATIONS } from '@/services/contract'
import { PERSON_OPERATIONS } from '@/services/person'
import { PROPERTY_OPERATIONS } from '@/services/property'
import { SHOWING_OPERATIONS } from '@/services/showing'
import { formCoreServices, formShowingService } from './form-service-runtime'
import { formServiceContext, serviceValue } from './service-binding-core'

export type FormServiceHydration = {
  fieldValues: Record<string, string>
  sections: Record<string, string>
}

function factString(value: unknown): string | null {
  if (typeof value !== 'string') return null
  const next = value.trim()
  return next || null
}

async function identityHydration(
  form: Pick<FormInstance, 'personId' | 'propertyId'>,
): Promise<{ personName: string | null; propertyName: string | null }> {
  const context = formServiceContext()
  const [person, property] = await Promise.all([
    form.personId
      ? serviceValue(
          formCoreServices.person.execute({
            operation: PERSON_OPERATIONS.GET,
            payload: { personId: form.personId },
            context,
          }),
          'Form Person hydration failed',
        )
      : Promise.resolve(null),
    form.propertyId
      ? serviceValue(
          formCoreServices.property.execute({
            operation: PROPERTY_OPERATIONS.GET,
            payload: { propertyId: form.propertyId },
            context: formServiceContext(),
          }),
          'Form Property hydration failed',
        )
      : Promise.resolve(null),
  ])

  return {
    personName: person?.displayName?.trim() || null,
    propertyName: property?.localName?.trim() || property?.displayName?.trim() || null,
  }
}

async function hydrateShowingReport(
  form: FormInstance,
  fieldValues: Record<string, string>,
  sections: Record<string, string>,
): Promise<FormServiceHydration> {
  const nextFields = { ...fieldValues }
  const nextSections = { ...sections }
  const identity = await identityHydration(form)

  // These are explicit template source bindings: Person and Property remain
  // canonical identity truth rather than stale report labels.
  if (identity.personName) nextFields.visitorName = identity.personName
  if (identity.propertyName) nextFields.property = identity.propertyName

  const showingId = await getFormShowingId(form.id)
  if (!showingId) return { fieldValues: nextFields, sections: nextSections }

  const showing = await serviceValue(
    formShowingService.execute({
      operation: SHOWING_OPERATIONS.GET,
      payload: { showingId },
      context: formServiceContext(),
    }),
    'Showing Report hydration failed',
  )
  if (!showing) return { fieldValues: nextFields, sections: nextSections }

  if (showing.showingDate) nextFields.showingDate = showing.showingDate
  if (showing.duration) nextFields.duration = showing.duration
  if (showing.outcome) nextFields.outcome = showing.outcome
  if (showing.interestScore !== null) {
    nextFields.feedbackScore = String(showing.interestScore)
  }
  if (showing.feedback) nextSections.feedback = showing.feedback
  if (showing.followUp) nextSections.followUp = showing.followUp

  return { fieldValues: nextFields, sections: nextSections }
}

async function hydrateOffer(
  form: FormInstance,
  fieldValues: Record<string, string>,
  sections: Record<string, string>,
): Promise<FormServiceHydration> {
  const nextFields = { ...fieldValues }
  const nextSections = { ...sections }
  const identity = await identityHydration(form)

  // OFFER-01 explicitly binds these two labels to Person/Property services.
  if (identity.personName) nextFields.buyerName = identity.personName
  if (identity.propertyName) nextFields.property = identity.propertyName

  const contractId = await getFormContractId(form.id)
  if (!contractId) return { fieldValues: nextFields, sections: nextSections }

  const contract = await serviceValue(
    formCoreServices.contract.execute({
      operation: CONTRACT_OPERATIONS.GET,
      payload: { contractId },
      context: formServiceContext(),
    }),
    'Offer Contract hydration failed',
  )
  if (!contract) return { fieldValues: nextFields, sections: nextSections }

  // Person/Property own buyerName/property. Contract owns the agreement facts.
  for (const name of [
    'sellerName',
    'brokerName',
    'offerAmount',
    'deposit',
    'financing',
    'closingDate',
    'expiration',
    'contingencies',
  ] as const) {
    const value = factString(contract.facts[name])
    if (value) nextFields[name] = value
  }
  const specialTerms = factString(contract.facts.specialTerms)
  if (specialTerms) nextSections.specialTerms = specialTerms

  return { fieldValues: nextFields, sections: nextSections }
}

/**
 * Compose service-owned truth into the mature Forms editor without creating a
 * second presentation model. Loading is read-only; explicit Save remains the
 * write-back boundary.
 */
export async function hydrateServiceBoundForm(
  form: FormInstance,
  fieldValues: Record<string, string> = form.fieldValues,
  sections: Record<string, string> = form.sections,
): Promise<FormServiceHydration> {
  if (form.templateId === 'SHOW-RPT') {
    return hydrateShowingReport(form, fieldValues, sections)
  }
  if (form.templateId === 'OFFER-01') {
    return hydrateOffer(form, fieldValues, sections)
  }
  return { fieldValues, sections }
}
