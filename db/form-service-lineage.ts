import type { QueryExecutor } from './query-executor'

async function executor(): Promise<QueryExecutor> {
  return (await import('./client')).sql
}

export type DirectFormContext = {
  personId: string
  propertyId: string
}

export async function resolveDealLaunchContext(
  dealId: string,
  execute?: QueryExecutor,
): Promise<DirectFormContext | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select
      coalesce(d.client_person_id, client.person_id) as person_id,
      d.property_id
    from deal d
    left join lateral (
      select dp.person_id
      from deal_participant dp
      where dp.deal_id = d.id
        and dp.role = 'client'
        and dp.active = true
      order by dp.created_at asc
      limit 1
    ) client on true
    where d.id = ${dealId}
    limit 1
  `
  const row = rows[0] as { person_id?: unknown; property_id?: unknown } | undefined
  if (!row?.person_id || !row?.property_id) return null
  return { personId: String(row.person_id), propertyId: String(row.property_id) }
}

export async function bindFormInstanceToDirectContext(
  input: { formInstanceId: string; personId: string; propertyId: string },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const rows = await q`
    update document_form_instance
    set person_id = ${input.personId},
        property_id = ${input.propertyId},
        deal_id = null,
        updated_at = now()
    where id = ${input.formInstanceId}
      and (person_id is null or person_id = ${input.personId})
      and (property_id is null or property_id = ${input.propertyId})
    returning id
  `
  if (!rows[0]) {
    throw new Error(
      'Form instance was not found or its explicit Person/Property context conflicts with the requested binding.',
    )
  }
}

/**
 * V4 Listing client selection keeps legacy Deal/document lineage intact while
 * explicitly changing the mutable draft's canonical Person/Property context.
 * Issued/snapshotted Listing Agreements can never be rebound.
 */
export async function bindListingFormContext(
  input: {
    formInstanceId: string
    personId: string
    propertyId: string | null
  },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const rows = await q`
    update document_form_instance f
    set person_id = ${input.personId},
        property_id = ${input.propertyId},
        updated_at = now()
    where f.id = ${input.formInstanceId}
      and f.template_id = 'LISTING-01'
      and f.status <> 'issued'
      and not exists (
        select 1
        from transaction_document td
        where td.form_instance_id = f.id
          and td.source = 'generated'
      )
    returning f.id
  `
  if (!rows[0]) {
    throw new Error(
      'Listing Agreement is not a mutable draft or already has issued document history.',
    )
  }
}

export async function getFormShowingId(
  formInstanceId: string,
  execute?: QueryExecutor,
): Promise<string | null> {
  const q = execute ?? (await executor())
  const rows = await q`
    select showing_id
    from document_form_instance
    where id = ${formInstanceId}
    limit 1
  `
  const row = rows[0] as { showing_id?: unknown } | undefined
  return row?.showing_id ? String(row.showing_id) : null
}

export async function bindFormInstanceToShowing(
  input: { formInstanceId: string; showingId: string },
  execute?: QueryExecutor,
): Promise<void> {
  const q = execute ?? (await executor())
  const rows = await q`
    update document_form_instance
    set showing_id = ${input.showingId}, updated_at = now()
    where id = ${input.formInstanceId}
      and (showing_id is null or showing_id = ${input.showingId})
    returning id
  `
  if (!rows[0]) {
    throw new Error(
      'Form instance was not found or is already bound to a different Showing.',
    )
  }
}
