import { sql } from "./client"
import type { QueryExecutor } from "./query-executor"

// ---------------------------------------------------------------------------
// SUPPORT-2 — Apple Contacts imported-contact read model (l_person projection).
//
// Server-side search + pagination over the relational load rows so the browser
// never receives the full 2,573-row payload. These rows are LOAD projections,
// NOT canonical CRM Clients: they must never be treated as canonical person /
// person_identity.
// ---------------------------------------------------------------------------

export type ImportedContact = {
  id: string
  source: string
  sourceAccount: string
  sourceContactId: string
  sourceRevision: number
  displayName: string
  organization: string | null
  displayAddress: string | null
  email: string | null
  phone: string | null
  reconciliationStatus: string
  projectedAt: string
}

export type ImportedContactsResult = {
  rows: ImportedContact[]
  total: number
  page: number
  pageSize: number
}

const SOURCE = "apple_contacts"

type ImportedContactRow = {
  id: string
  source: string
  source_account: string
  source_contact_id: string
  source_revision: number
  display_name: string
  organization: string | null
  display_address: string | null
  email: string | null
  phone: string | null
  reconciliation_status: string
  projected_at: string
}

function mapRow(row: ImportedContactRow): ImportedContact {
  return {
    id: row.id,
    source: row.source,
    sourceAccount: row.source_account,
    sourceContactId: row.source_contact_id,
    sourceRevision: Number(row.source_revision),
    displayName: row.display_name,
    organization: row.organization ?? null,
    displayAddress: row.display_address ?? null,
    email: row.email ?? null,
    phone: row.phone ?? null,
    reconciliationStatus: row.reconciliation_status,
    projectedAt: new Date(row.projected_at).toISOString(),
  }
}

export async function getImportedContacts(
  opts: { search?: string; page?: number; pageSize?: number },
  execute?: QueryExecutor,
): Promise<ImportedContactsResult> {
  const q = execute ?? sql
  const search = (opts.search ?? "").trim()
  const page = Math.max(1, opts.page ?? 1)
  const pageSize = Math.max(1, Math.min(50, opts.pageSize ?? 25))
  const offset = (page - 1) * pageSize
  // null = no search (branch into a search-free query; never pass null params).
  const like = search === "" ? null : `%${search}%`

  const countRows =
    like === null
      ? ((await q`
          select count(*)::int as total
          from l_person lp
          where lp.source = ${SOURCE}
        `) as { total: number }[])
      : ((await q`
          select count(*)::int as total
          from l_person lp
          where lp.source = ${SOURCE}
            and (lp.display_name ilike ${like}
              or lp.organization ilike ${like}
              or lp.display_address ilike ${like}
              or lp.source_contact_id ilike ${like}
              or exists (select 1 from l_person_identity li2 where li2.l_person_id = lp.id and li2.identity_value ilike ${like}))
        `) as { total: number }[])
  const total = Number(countRows[0]?.total ?? 0)

  const rows =
    like === null
      ? ((await q`
          select
            lp.id, lp.source, lp.source_account, lp.source_contact_id, lp.source_revision,
            lp.display_name, lp.organization, lp.display_address, lp.reconciliation_status,
            lp.projected_at,
            (select identity_value from l_person_identity li
               where li.l_person_id = lp.id and li.identity_type = 'email'
               order by li.is_primary desc, li.ordinal asc, li.created_at asc limit 1) as email,
            (select identity_value from l_person_identity li
               where li.l_person_id = lp.id and li.identity_type = 'phone'
               order by li.is_primary desc, li.ordinal asc, li.created_at asc limit 1) as phone
          from l_person lp
          where lp.source = ${SOURCE}
          order by lp.display_name asc, lp.id asc
          limit ${pageSize} offset ${offset}
        `) as ImportedContactRow[])
      : ((await q`
          select
            lp.id, lp.source, lp.source_account, lp.source_contact_id, lp.source_revision,
            lp.display_name, lp.organization, lp.display_address, lp.reconciliation_status,
            lp.projected_at,
            (select identity_value from l_person_identity li
               where li.l_person_id = lp.id and li.identity_type = 'email'
               order by li.is_primary desc, li.ordinal asc, li.created_at asc limit 1) as email,
            (select identity_value from l_person_identity li
               where li.l_person_id = lp.id and li.identity_type = 'phone'
               order by li.is_primary desc, li.ordinal asc, li.created_at asc limit 1) as phone
          from l_person lp
          where lp.source = ${SOURCE}
            and (lp.display_name ilike ${like}
              or lp.organization ilike ${like}
              or lp.display_address ilike ${like}
              or lp.source_contact_id ilike ${like}
              or exists (select 1 from l_person_identity li2 where li2.l_person_id = lp.id and li2.identity_value ilike ${like}))
          order by lp.display_name asc, lp.id asc
          limit ${pageSize} offset ${offset}
        `) as ImportedContactRow[])

  return { rows: rows.map(mapRow), total, page, pageSize }
}

export async function getImportedContactsCount(execute?: QueryExecutor): Promise<number> {
  const q = execute ?? sql
  const rows = (await q`
    select count(*)::int as total from l_person where source = ${SOURCE}
  `) as { total: number }[]
  return Number(rows[0]?.total ?? 0)
}
