import type { FormInstance } from '@/db/document-form-instance'
import { getFormInstance } from '@/db/document-form-instance'
import {
  bindFormInstanceToDirectContext,
  resolveDealLaunchContext,
} from '@/db/form-service-lineage'
import { syncOfferContractForm } from './offer-contract-binding'
import { syncShowingReportForm } from './showing-report-binding'

export const SERVICE_BOUND_FORM_TEMPLATES = new Set(['SHOW-RPT', 'OFFER-01'])

export function isServiceBoundFormTemplate(templateId: string): boolean {
  return SERVICE_BOUND_FORM_TEMPLATES.has(templateId)
}

async function ensureDirectContext(form: FormInstance): Promise<FormInstance> {
  let personId = form.personId
  let propertyId = form.propertyId

  if ((!personId || !propertyId) && form.dealId) {
    const launch = await resolveDealLaunchContext(form.dealId)
    personId = personId ?? launch?.personId ?? null
    propertyId = propertyId ?? launch?.propertyId ?? null
  }

  if (!personId || !propertyId) {
    throw new Error(
      `${form.templateId} requires an explicit Client/Person and Property.`,
    )
  }

  if (form.dealId || form.personId !== personId || form.propertyId !== propertyId) {
    await bindFormInstanceToDirectContext({
      formInstanceId: form.id,
      personId,
      propertyId,
    })
  }

  return {
    ...form,
    dealId: null,
    personId,
    propertyId,
  }
}

export type FormServiceBindingOverrides = {
  fieldValues?: Record<string, string>
  sections?: Record<string, string>
}

export type FormServiceBindingResult =
  | { kind: 'none' }
  | { kind: 'showing'; showingId: string }
  | { kind: 'contract'; contractId: string }

export async function syncFormServiceBinding(
  formId: string,
  actorId: string | null = null,
  overrides: FormServiceBindingOverrides = {},
): Promise<FormServiceBindingResult> {
  const stored = await getFormInstance(formId)
  if (!stored) throw new Error(`Form instance not found: ${formId}`)
  if (!isServiceBoundFormTemplate(stored.templateId)) return { kind: 'none' }

  const direct = await ensureDirectContext(stored)
  const form = {
    ...direct,
    fieldValues: overrides.fieldValues ?? direct.fieldValues,
    sections: overrides.sections ?? direct.sections,
  }

  if (form.templateId === 'SHOW-RPT') {
    return {
      kind: 'showing',
      showingId: await syncShowingReportForm(form, actorId),
    }
  }

  return {
    kind: 'contract',
    contractId: await syncOfferContractForm(form, actorId),
  }
}
