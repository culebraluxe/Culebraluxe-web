import { PortalWriteError } from '../lib/portal-write-error'
import type { QueryExecutor, QueryRow } from './query-executor'
import {
  bindFormInstanceToDirectContext,
  bindFormInstanceToShowing,
  bindListingFormContext,
  getFormShowingId,
  resolveDealLaunchContext,
} from './form-service-lineage'
import { listFormSignerPeople } from './form-signer'
import type { FormSignerPerson } from '../lib/forms/signer-resolution'
import type {
  BindFormInstanceToDirectContextRequest,
  BindFormInstanceToShowingRequest,
  BindListingFormContextRequest,
  CreateFormInstanceInput,
  CreateFormInstanceRequest,
  DealFormFacts,
  DirectFormContext,
  FormInstance,
  FormInstanceEvidence,
  FormInstanceListItem,
  FormInstanceStatus,
  FormRepository,
  GetDealFormFactsRequest,
  GetFormInstanceRequest,
  GetFormShowingIdRequest,
  LatestFormEvidenceRequest,
  ListFormSignerPeopleRequest,
  ResolveDealLaunchContextRequest,
  SeedFormParticipantsRequest,
  UpdateFormInstanceInput,
  UpdateFormInstanceRequest,
} from '@/services/forms'

// ---------------------------------------------------------------------------
// FORMS — form instance repository (migration 054).
//
// MUTABLE working state. A form instance is the editable assembly of a
// TemplateDefinition against a deal: template identity, structured field
// values and bounded editable prose sections. It is NEVER the immutable
// business record — issuance snapshots these values into a NEW
// transaction_document row (DOC-06) and marks the instance 'issued'.
//
// Reachable only through the Forms service via SqlFormInstanceRepository.
// ---------------------------------------------------------------------------

export type FormInstanceRow = QueryRow & {
  id: string
  template_id: string
  template_version: number
  deal_id: string | null
  person_id: string | null
  property_id: string | null
  contract_id: string | null
  status: string
  field_values: unknown
  sections: unknown
  created_by_user_id: string | null
  created_at: unknown
  updated_at: unknown
}

function mapFormInstance(row: FormInstanceRow): FormInstance {
  return {
    id: row.id,
    templateId: row.template_id,
    templateVersion: Number(row.template_version),
    dealId: row.deal_id ?? null,
    personId: row.person_id ?? null,
    propertyId: row.property_id ?? null,
    contractId: row.contract_id ?? null,
    status: row.status as FormInstanceStatus,
    fieldValues: (row.field_values ?? {}) as Record<string, string>,
    sections: (row.sections ?? {}) as Record<string, string>,
    createdByUserId: row.created_by_user_id ?? null,
    createdAt: new Date(row.created_at as string).toISOString(),
    updatedAt: new Date(row.updated_at as string).toISOString(),
  }
}

let defaultExecutor: QueryExecutor | null = null

async function executor(): Promise<QueryExecutor> {
  if (!defaultExecutor) {
    const client = await import('./client')
    defaultExecutor = client.sql
  }
  return defaultExecutor
}

export async function createFormInstance(
  input: CreateFormInstanceInput,
  execute?: QueryExecutor,
): Promise<FormInstance> {
  if (!input.templateId.trim()) {
    throw new PortalWriteError('validation', 'templateId is required.')
  }
  const dealId = input.dealId?.trim() || null
  const personId = input.personId?.trim() || null
  const propertyId = input.propertyId?.trim() || null
  if (!dealId && !personId && !propertyId) {
    throw new PortalWriteError(
      'validation',
      'A deal, client, or property is required.',
    )
  }
  const q = execute ?? (await executor())
  const rows = await q`
    insert into document_form_instance (
      template_id, template_version, deal_id, person_id, property_id,
      field_values, sections, created_by_user_id
    ) values (
      ${input.templateId}, ${input.templateVersion}, ${dealId},
      ${personId}, ${propertyId},
      ${JSON.stringify(input.fieldValues)}::jsonb,
      ${JSON.stringify(input.sections)}::jsonb,
      ${input.createdByUserId ?? null}
    )
    returning id, template_id, template_version, deal_id, person_id, property_id, contract_id,
      status, field_values, sections, created_by_user_id, created_at, updated_at
  `
  const row = rows[0] as FormInstanceRow | undefined
  if (!row) {
    throw new PortalWriteError('not-found', 'Form instance creation returned no row.')
  }
  return mapFormInstance(row)
}

// __PART2__
export async function getFormInstance(
  id: string,
  execute?: QueryExecutor,
): Promise<FormInstance | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select id, template_id, template_version, deal_id, person_id, property_id, contract_id, status,
      field_values, sections, created_by_user_id, created_at, updated_at
    from document_form_instance
    where id = ${id}
    limit 1
  `
  const row = rows[0] as FormInstanceRow | undefined
  return row ? mapFormInstance(row) : null
}

export async function updateFormInstance(
  id: string,
  input: UpdateFormInstanceInput,
  execute?: QueryExecutor,
): Promise<FormInstance | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    update document_form_instance
    set field_values = case
          when ${input.fieldValues ? JSON.stringify(input.fieldValues) : null}::jsonb is null
            then field_values else ${JSON.stringify(input.fieldValues ?? {})}::jsonb end,
        sections = case
          when ${input.sections ? JSON.stringify(input.sections) : null}::jsonb is null
            then sections else ${JSON.stringify(input.sections ?? {})}::jsonb end,
        status = case when ${input.status ?? null}::text is null
          then status else ${input.status ?? 'draft'} end,
        contract_id = case when ${input.contractId ?? null}::uuid is null
          then contract_id else ${input.contractId ?? null}::uuid end,
        updated_at = now()
    where id = ${id}
    returning id, template_id, template_version, deal_id, person_id, property_id, contract_id, status,
      field_values, sections, created_by_user_id, created_at, updated_at
  `
  const row = rows[0] as FormInstanceRow | undefined
  return row ? mapFormInstance(row) : null
}

/**
 * Resolve the bounded canonical facts used to prefill a form instance for a
 * deal. PURE read; never mutates. Missing facts stay null — the POC does not
 * broaden domain models merely to fill a form. The client is resolved through
 * the canonical deal_participant active-client row (mirrors the Deal Workspace
 * projection); property + deal facts come from the deal/property rows.
 */
export async function getDealFormFacts(
  dealId: string,
  execute?: QueryExecutor,
): Promise<DealFormFacts | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select d.offer_price, d.closing_date, d.financing_type,
      p.name as property_name, p.location as property_location,
      client.display_name as client_name
    from deal d
    left join property p on p.id = d.property_id
    join lateral (
      select person.id, person.display_name
      from deal_participant dp
      join person on person.id = dp.person_id
      where dp.deal_id = d.id
        and dp.role = 'client'
        and dp.active
      order by dp.created_at asc
      limit 1
    ) client on true
    where d.id = ${dealId}
    limit 1
  `
  const row = rows[0]
  if (!row) return null
  const rawFinancing = row.financing_type as string | null
  return {
    clientName: (row.client_name as string | null) ?? null,
    personDisplayName: (row.client_name as string | null) ?? null,
    propertyName: (row.property_name as string | null) ?? null,
    propertyLocation: (row.property_location as string | null) ?? null,
    propertyLabel: (row.property_name as string | null) ?? null,
    offerAmount:
      row.offer_price === null || row.offer_price === undefined
        ? null
        : String(row.offer_price),
    financingType:
      rawFinancing === null || rawFinancing === undefined
        ? null
        : rawFinancing === 'cash'
          ? 'Cash'
          : 'Financed',
    closingDate:
      row.closing_date === null || row.closing_date === undefined
        ? null
        : new Date(row.closing_date as string).toISOString().slice(0, 10),
  }
}


const ROLE_MAP: Record<string, string> = {
  client: 'BUYER',
  seller: 'SELLER',
  owner: 'SELLER',
}

export async function seedFormParticipantsFromDeal(
  formInstanceId: string,
  dealId: string,
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const rows = await q`
    select dp.role, dp.person_id, coalesce(person.display_name, dp.role) as display_name
    from deal_participant dp
    left join person on person.id = dp.person_id
    where dp.deal_id = ${dealId}
      and dp.active = true
    order by dp.created_at asc
  `
  let order = 0
  for (const row of rows) {
    const role = ROLE_MAP[String(row.role)] ?? 'OTHER'
    await q`
      insert into document_form_participant (form_instance_id, role, person_id, display_name, sort_order)
      values (
        ${formInstanceId},
        ${role},
        ${row.person_id ? String(row.person_id) : null},
        ${String(row.display_name)},
        ${order}
      )
    `
    order += 1
  }
}

export async function listFormInstances(
  execute?: QueryExecutor,
): Promise<FormInstanceListItem[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select f.id, f.template_id, f.template_version, f.deal_id, f.person_id, f.property_id, f.contract_id, f.status,
      f.field_values, f.sections, f.created_by_user_id, f.created_at, f.updated_at,
      null as deal_label,
      coalesce(p.name, fp.name) as property_label,
      coalesce(c.display_name, person.display_name) as client_name
    from document_form_instance f
    left join deal d on d.id = f.deal_id
    left join property p on p.id = d.property_id
    left join person c on c.id = d.client_person_id
    left join person on person.id = f.person_id
    left join property fp on fp.id = f.property_id
    order by f.updated_at desc, f.id
  `
  return rows.map((row) => ({
    ...mapFormInstance(row as FormInstanceRow),
    dealLabel: (row.deal_label as string | null) ?? null,
    propertyLabel: (row.property_label as string | null) ?? null,
    clientName: (row.client_name as string | null) ?? null,
  }))
}

export async function latestFormEvidence(
  templateId: string,
  personId: string,
  roles: readonly string[] | null,
  execute?: QueryExecutor,
): Promise<FormInstanceEvidence | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select f.id, f.property_id, f.field_values, f.updated_at
    from document_form_instance f
    left join deal d on d.id = f.deal_id
    where f.template_id = ${templateId}
      and (
        f.person_id = ${personId}
        or d.client_person_id = ${personId}
        or exists (
          select 1
          from deal_participant dp
          where dp.deal_id = f.deal_id
            and dp.person_id = ${personId}
            and dp.active = true
            and (${roles}::text[] is null or dp.role = any(${roles}::text[]))
        )
      )
    order by f.updated_at desc, f.id desc
    limit 1
  `
  const row = rows[0]
  if (!row) return null
  return {
    formInstanceId: String(row.id),
    propertyId: row.property_id ? String(row.property_id) : null,
    fieldValues: (row.field_values ?? {}) as Record<string, string>,
    updatedAt: row.updated_at ? new Date(row.updated_at as string).toISOString() : null,
  }
}

/**
 * The persistence boundary the Forms service talks to. Nothing outside a
 * repository queries document_form_instance / document_form_participant.
 */
export class SqlFormInstanceRepository implements FormRepository {
  constructor(private readonly execute?: QueryExecutor) {}

  createInstance(request: CreateFormInstanceRequest): Promise<FormInstance> {
    return createFormInstance(request, this.execute)
  }

  getInstance(request: GetFormInstanceRequest): Promise<FormInstance | null> {
    return getFormInstance(request.formInstanceId, this.execute)
  }

  updateInstance(request: UpdateFormInstanceRequest): Promise<FormInstance | null> {
    return updateFormInstance(request.formInstanceId, request.input, this.execute)
  }

  listInstances(): Promise<FormInstanceListItem[]> {
    return listFormInstances(this.execute)
  }

  dealFacts(request: GetDealFormFactsRequest): Promise<DealFormFacts | null> {
    return getDealFormFacts(request.dealId, this.execute)
  }

  seedParticipantsFromDeal(request: SeedFormParticipantsRequest): Promise<void> {
    return seedFormParticipantsFromDeal(request.formInstanceId, request.dealId, this.execute)
  }

  latestEvidence(request: LatestFormEvidenceRequest): Promise<FormInstanceEvidence | null> {
    return latestFormEvidence(
      request.templateId,
      request.personId,
      request.roles ? [...request.roles] : null,
      this.execute,
    )
  }

  // ---- Lineage (repository-internal SQL modules; reachable only through here) ----

  resolveDealLaunchContext(
    request: ResolveDealLaunchContextRequest,
  ): Promise<DirectFormContext | null> {
    return resolveDealLaunchContext(request.dealId, this.execute)
  }

  bindDirectContext(request: BindFormInstanceToDirectContextRequest): Promise<void> {
    return bindFormInstanceToDirectContext(request, this.execute)
  }

  bindListingContext(request: BindListingFormContextRequest): Promise<void> {
    return bindListingFormContext(request, this.execute)
  }

  getShowingId(request: GetFormShowingIdRequest): Promise<string | null> {
    return getFormShowingId(request.formInstanceId, this.execute)
  }

  bindShowing(request: BindFormInstanceToShowingRequest): Promise<void> {
    return bindFormInstanceToShowing(request, this.execute)
  }

  listSignerPeople(request: ListFormSignerPeopleRequest): Promise<FormSignerPerson[]> {
    return listFormSignerPeople(request.formInstanceId, this.execute)
  }
}

