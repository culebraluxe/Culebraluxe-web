import type { FormInstance } from '@/db/document-form-instance'
import { getFormInstance } from '@/db/document-form-instance'
import { sql } from '@/db/client'
import {
  bindFormInstanceToDirectContext,
  resolveDealLaunchContext,
} from '@/db/form-service-lineage'
import {
  loadListingCanonicalSnapshot,
  saveListingCanonicalFields,
} from './listing-canonical-binding'
import {
  LISTING_CANONICAL_FIELD_NAMES,
  type ListingCanonicalFields,
} from './listing-field-binding'
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
  | { kind: 'listing'; personId: string; physicalPropertyId: string | null }
  | { kind: 'showing'; showingId: string }
  | { kind: 'contract'; contractId: string }

/**
 * LISTING-01 was built before the Person <-> Property migrations were promoted
 * everywhere. Keep the mature Forms workflow usable during rollout; once 115
 * and 116 exist in the target database the exact same editor save path begins
 * synchronizing the reviewed Listing fields through PersonService/PropertyService.
 */
async function listingCanonicalSchemaReady(): Promise<boolean> {
  const rows = await sql`
    select (
      to_regclass('public.person_property') is not null
      and exists (
        select 1 from information_schema.columns
        where table_schema = 'public' and table_name = 'property' and column_name = 'legal_owner_name'
      )
      and exists (
        select 1 from information_schema.columns
        where table_schema = 'public' and table_name = 'property' and column_name = 'country'
      )
      and exists (
        select 1 from information_schema.columns
        where table_schema = 'public' and table_name = 'property' and column_name = 'iso_country_code'
      )
      and exists (
        select 1 from information_schema.columns
        where table_schema = 'public' and table_name = 'property' and column_name = 'address_line1'
      )
    ) as ready
  `
  return rows[0]?.ready === true
}

async function syncListingForm(
  stored: FormInstance,
  actorId: string | null,
  overrides: FormServiceBindingOverrides,
): Promise<FormServiceBindingResult> {
  if (!(await listingCanonicalSchemaReady())) return { kind: 'none' }

  let personId = stored.personId
  let propertyId = stored.propertyId
  if ((!personId || !propertyId) && stored.dealId) {
    const launch = await resolveDealLaunchContext(stored.dealId)
    personId = personId ?? launch?.personId ?? null
    propertyId = propertyId ?? launch?.propertyId ?? null
  }

  if (!personId) {
    throw new Error('LISTING-01 requires a canonical Client/Person before service synchronization.')
  }

  const before = await loadListingCanonicalSnapshot(personId)
  const values = overrides.fieldValues ?? stored.fieldValues
  const fields = Object.fromEntries(
    LISTING_CANONICAL_FIELD_NAMES.map((name) => [
      name,
      Object.prototype.hasOwnProperty.call(values, name)
        ? values[name] ?? ''
        : before.fields[name] ?? '',
    ]),
  ) as ListingCanonicalFields

  // Preserve hidden canonical qualifiers (for example legalOwnerName on the
  // immutable LISTING-01 v4 template) instead of clearing them simply because
  // the production editor does not render a field that the template does not own.
  const result = await saveListingCanonicalFields(
    personId,
    fields,
    actorId,
    propertyId ?? before.physicalPropertyId ?? undefined,
  )

  return {
    kind: 'listing',
    personId,
    physicalPropertyId: result.physicalPropertyId,
  }
}

export async function syncFormServiceBinding(
  formId: string,
  actorId: string | null = null,
  overrides: FormServiceBindingOverrides = {},
): Promise<FormServiceBindingResult> {
  const stored = await getFormInstance(formId)
  if (!stored) throw new Error(`Form instance not found: ${formId}`)

  // Listing keeps its legacy Deal/document lineage intact. Only its six
  // Person/Property-owned fields round-trip through the canonical services.
  if (stored.templateId === 'LISTING-01') {
    return syncListingForm(stored, actorId, overrides)
  }

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
