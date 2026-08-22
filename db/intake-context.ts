import { sql } from './client'

import type {
  ResolvedDeal,
  ResolvedProperty,
} from '../lib/crm-intake-types'
import type { QueryExecutor } from './query-executor'

type PropertyRow = {
  id: string
  slug: string | null
}

function mapProperty(row: PropertyRow): ResolvedProperty {
  return { id: row.id, slug: row.slug ?? undefined }
}

export async function findPropertyById(
  propertyId: string,
  execute: QueryExecutor = sql,
): Promise<ResolvedProperty | null> {
  const rows = await execute`
    select id, slug
    from property
    where id = ${propertyId}
      and archived_at is null
    limit 1
  `
  const row = rows[0] as PropertyRow | undefined
  return row ? mapProperty(row) : null
}

export async function findPropertyBySlug(
  slug: string,
  execute: QueryExecutor = sql,
): Promise<ResolvedProperty | null> {
  const rows = await execute`
    select id, slug
    from property
    where slug = ${slug}
      and archived_at is null
    limit 1
  `
  const row = rows[0] as PropertyRow | undefined
  return row ? mapProperty(row) : null
}

export async function findDealById(
  dealId: string,
  execute: QueryExecutor = sql,
): Promise<ResolvedDeal | null> {
  // The deal's client is the active deal_participant role='client' row — the
  // canonical participant model (CRM-13). The legacy client FK remains a
  // denormalized mirror, not the read source.
  const rows = await execute`
    select
      d.id,
      d.property_id,
      (
        select dp.person_id
        from deal_participant dp
        where dp.deal_id = d.id
          and dp.role = 'client'
          and dp.active = true
        order by dp.created_at asc
        limit 1
      ) as client_person_id
    from deal d
    where d.id = ${dealId}
    limit 1
  `
  const row = rows[0] as
    | { id: string; client_person_id: string | null; property_id: string }
    | undefined

  if (!row) return null

  return {
    id: row.id,
    personId: row.client_person_id,
    propertyId: row.property_id,
  }
}
