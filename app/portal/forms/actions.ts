'use server'

import { resolveDealLaunchContext } from '@/db/form-service-lineage'
import {
  isServiceBoundFormTemplate,
  syncFormServiceBinding,
} from '@/lib/forms/form-service-binding'
import {
  createFormAction as coreCreateFormAction,
  grokFillFormAction as coreGrokFillFormAction,
  issueFormAction as coreIssueFormAction,
  sendFormForSignatureAction as coreSendFormForSignatureAction,
  updateFormAction as coreUpdateFormAction,
  type FormActionResult,
  type FormSignatureSendData,
} from './actions-core'

export type { FormActionResult, FormSignatureSendData } from './actions-core'

function bindingFail<T>(message: string): FormActionResult<T> {
  return { ok: false, code: 'validation', message }
}

async function directCreateContext(input: {
  templateId: string
  dealId?: string
  personId?: string
  propertyId?: string
}) {
  if (!isServiceBoundFormTemplate(input.templateId)) return input

  let personId = input.personId?.trim() || null
  let propertyId = input.propertyId?.trim() || null
  const dealId = input.dealId?.trim() || null
  if ((!personId || !propertyId) && dealId) {
    const launch = await resolveDealLaunchContext(dealId)
    personId = personId ?? launch?.personId ?? null
    propertyId = propertyId ?? launch?.propertyId ?? null
  }

  if (!personId || !propertyId) return null
  return {
    templateId: input.templateId,
    personId,
    propertyId,
  }
}

export async function createOfferLetterFormAction(
  dealId: string,
): Promise<FormActionResult<{ formId: string }>> {
  return createFormAction({ templateId: 'OFFER-01', dealId })
}

export async function createFormAction(input: {
  templateId: string
  dealId?: string
  personId?: string
  propertyId?: string
}): Promise<FormActionResult<{ formId: string }>> {
  try {
    const normalized = await directCreateContext(input)
    if (!normalized) {
      return bindingFail(
        `${input.templateId} requires both a Client/Person and a Property.`,
      )
    }
    const result = await coreCreateFormAction(normalized)
    if (!result.ok) return result

    // LISTING-01 creation is intentionally non-mutating. The canonical Person /
    // Property values hydrate the working editor, and the first explicit Save
    // is the review boundary that may synchronize those six owned fields.
    if (normalized.templateId !== 'LISTING-01') {
      await syncFormServiceBinding(result.data.formId)
    }
    return result
  } catch (error) {
    console.error('Form service binding failed during create.', error)
    return bindingFail('Could not bind the form to its canonical service context.')
  }
}

export async function updateFormAction(
  formId: string,
  fieldValues: Record<string, string>,
  sections: Record<string, string>,
): Promise<FormActionResult<{ updated: boolean }>> {
  const result = await coreUpdateFormAction(formId, fieldValues, sections)
  if (!result.ok) return result
  try {
    await syncFormServiceBinding(formId)
    return result
  } catch (error) {
    console.error('Form service binding failed during save.', error)
    return bindingFail('The form was saved, but its service-side draft could not be synchronized.')
  }
}

export async function issueFormAction(
  formId: string,
): Promise<FormActionResult<{ documentId: string; issuedVersion: number; checksum: string }>> {
  try {
    await syncFormServiceBinding(formId)
  } catch (error) {
    console.error('Form service binding failed before issue.', error)
    return bindingFail('The form could not be synchronized to its canonical service before issue.')
  }
  return coreIssueFormAction(formId)
}

export async function grokFillFormAction(
  input: Parameters<typeof coreGrokFillFormAction>[0],
) {
  return coreGrokFillFormAction(input)
}

export async function sendFormForSignatureAction(
  formId: string,
  input: Parameters<typeof coreSendFormForSignatureAction>[1],
): Promise<FormActionResult<FormSignatureSendData>> {
  try {
    // The core send action persists these same values when they differ. Binding
    // from the submitted draft first keeps service truth aligned with the PDF
    // that is about to be issued without changing the core envelope rules.
    await syncFormServiceBinding(formId, null, {
      fieldValues: input.fieldValues,
      sections: input.sections,
    })
  } catch (error) {
    console.error('Form service binding failed before signature send.', error)
    return bindingFail('The form could not be synchronized to its canonical service before sending.')
  }
  return coreSendFormForSignatureAction(formId, input)
}
