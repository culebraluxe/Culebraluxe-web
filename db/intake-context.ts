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
  const rows = await execute`
    select id, client_person_id, property_id
    from deal
    where id = ${dealId}
    limit 1
  `
  const row = rows[0] as
    | { id: string; client_person_id: string; property_id: string }
    | undefined

  return row
    ? {
        id: row.id,
        personId: row.client_person_id,
        propertyId: row.property_id,
      }
    : null
}
