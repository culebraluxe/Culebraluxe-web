import { sql } from './client'

// Narrow read-only person search for bounded operator selection. This is NOT
// identity resolution: it only returns already-existing canonical people by
// exact substring match on display name or recorded identity value. The
// operator explicitly chooses a person; no fuzzy matching, ranking, or
// suggestion semantics are introduced.

export type PersonSearchResult = {
  id: string
  displayName: string
  role: string
  status: string
  location: string | null
  email: string | null
  phone: string | null
}

type PersonSearchRow = {
  id: string
  display_name: string
  role: string
  status: string
  location: string | null
  email: string | null
  phone: string | null
}

export async function searchPeople(
  query: string,
  limit = 8,
): Promise<PersonSearchResult[]> {
  const q = query.trim()
  if (!q) return []

  const pattern = `%${q}%`

  const rows = await sql`
    select
      p.id,
      p.display_name,
      p.role,
      p.status,
      p.location,
      (
        select i.identity_value
        from person_identity i
        where i.person_id = p.id
          and i.identity_type = 'email'
        order by i.is_primary desc, i.created_at desc
        limit 1
      ) as email,
      (
        select i.identity_value
        from person_identity i
        where i.person_id = p.id
          and i.identity_type = 'phone'
        order by i.is_primary desc, i.created_at desc
        limit 1
      ) as phone
    from person p
    where p.archived_at is null
      and (
        p.display_name ilike ${pattern}
        or exists (
          select 1
          from person_identity i
          where i.person_id = p.id
            and i.identity_value ilike ${pattern}
        )
      )
    order by p.display_name asc
    limit ${limit}
  `

  return (rows as PersonSearchRow[]).map((row) => ({
    id: row.id,
    displayName: row.display_name,
    role: row.role,
    status: row.status,
    location: row.location ?? null,
    email: row.email ?? null,
    phone: row.phone ?? null,
  }))
}
