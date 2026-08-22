import { PortalWriteError } from '../lib/portal-write-error'
import type { QueryExecutor, QueryRow } from './query-executor'

// ---------------------------------------------------------------------------
// DOC-07 — form instance repository (migration 054).
//
// MUTABLE working state. A form instance is the editable assembly of a
// TemplateDefinition against a deal: template identity, structured field
// values and bounded editable prose sections. It is NEVER the immutable
// business record — issuance snapshots these values into a NEW
// transaction_document row (DOC-06) and marks the instance 'issued'.
// ---------------------------------------------------------------------------

export type FormInstanceStatus = 'draft' | 'ready' | 'issued'

export type FormInstance = {
  id: string
  templateId: string
  templateVersion: number
  dealId: string
  status: FormInstanceStatus
  fieldValues: Record<string, string>
  sections: Record<string, string>
  createdByUserId: string | null
  createdAt: string
  updatedAt: string
}

export type FormInstanceRow = QueryRow & {
  id: string
  template_id: string
  template_version: number
  deal_id: string
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
    dealId: row.deal_id,
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

export type CreateFormInstanceInput = {
  templateId: string
  templateVersion: number
  dealId: string
  fieldValues: Record<string, string>
  sections: Record<string, string>
  createdByUserId?: string | null
}

export async function createFormInstance(
  input: CreateFormInstanceInput,
  execute?: QueryExecutor,
): Promise<FormInstance> {
  if (!input.templateId.trim()) {
    throw new PortalWriteError('validation', 'templateId is required.')
  }
  if (!input.dealId.trim()) {
    throw new PortalWriteError('validation', 'dealId is required.')
  }
  const q = execute ?? (await executor())
  const rows = await q`
    insert into document_form_instance (
      template_id, template_version, deal_id, field_values, sections,
      created_by_user_id
    ) values (
      ${input.templateId}, ${input.templateVersion}, ${input.dealId},
      ${JSON.stringify(input.fieldValues)}::jsonb,
      ${JSON.stringify(input.sections)}::jsonb,
      ${input.createdByUserId ?? null}
    )
    returning id, template_id, template_version, deal_id, status,
      field_values, sections, created_by_user_id, created_at, updated_at
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
    select id, template_id, template_version, deal_id, status,
      field_values, sections, created_by_user_id, created_at, updated_at
    from document_form_instance
    where id = ${id}
    limit 1
  `
  const row = rows[0] as FormInstanceRow | undefined
  return row ? mapFormInstance(row) : null
}

export type UpdateFormInstanceInput = {
  fieldValues?: Record<string, string>
  sections?: Record<string, string>
  status?: FormInstanceStatus
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
        updated_at = now()
    where id = ${id}
    returning id, template_id, template_version, deal_id, status,
      field_values, sections, created_by_user_id, created_at, updated_at
  `
  const row = rows[0] as FormInstanceRow | undefined
  return row ? mapFormInstance(row) : null
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


export async function listFormInstances(
  execute?: QueryExecutor,
): Promise<FormInstanceListItem[]> {
  const q = execute ?? (await executor())
  const rows = await q`
    select f.id, f.template_id, f.template_version, f.deal_id, f.status,
      f.field_values, f.sections, f.created_by_user_id, f.created_at, f.updated_at,
      d.name as deal_label,
      p.name as property_label,
      c.display_name as client_name
    from document_form_instance f
    left join deal d on d.id = f.deal_id
    left join property p on p.id = d.property_id
    left join person c on c.id = d.client_id
    order by f.updated_at desc, f.id
  `
  return rows.map((row) => ({
    ...mapFormInstance(row as FormInstanceRow),
    dealLabel: (row.deal_label as string | null) ?? null,
    propertyLabel: (row.property_label as string | null) ?? null,
    clientName: (row.client_name as string | null) ?? null,
  }))
}

