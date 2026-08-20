import { sql } from './client'

// AUTH-01 read projections for the Portal Settings area (Stories 9-11).
// These query the canonical application security model:
//   app_user, role, authority, role_authority, app_user_role
// which are created by migration 015_auth_security_model.sql.
//
// Read-only. Requires migration 015 to be applied before these run.

export type SettingsUser = {
  id: string
  displayName: string
  email: string | null
  accountType: 'internal' | 'external'
  active: boolean
  personName: string | null
  roles: string[]
  createdAtLabel: string | null
}

export type SettingsRole = {
  id: string
  code: string
  name: string
  accountType: 'internal' | 'external'
  description: string | null
  active: boolean
  authorities: string[]
  userCount: number
}

export type SettingsAuthority = {
  id: string
  code: string
  name: string
  description: string | null
  roles: string[]
}

type SettingsUserRaw = {
  id: string
  display_name: string
  email: string | null
  account_type: 'internal' | 'external'
  active: boolean
  person_name: string | null
  created_at_label: string | null
  roles: string[] | null
}

export async function getSettingsUsers(): Promise<SettingsUser[]> {
  const rows = await sql`
    select
      u.id,
      u.display_name,
      u.email,
      u.account_type,
      u.active,
      person.display_name as person_name,
      to_char(
        u.created_at at time zone 'America/Puerto_Rico',
        'Mon FMDD, YYYY'
      ) as created_at_label,
      coalesce(
        (
          select jsonb_agg(r.code order by r.code)
          from app_user_role aur
          join role r
            on r.id = aur.role_id
          where aur.app_user_id = u.id
            and r.active = true
        ),
        '[]'::jsonb
      ) as roles
    from app_user u
    left join person
      on person.id = u.person_id
    order by u.active desc, u.display_name asc
  `

  return (rows as SettingsUserRaw[]).map((row) => ({
    id: row.id,
    displayName: row.display_name,
    email: row.email ?? null,
    accountType: row.account_type,
    active: row.active,
    personName: row.person_name ?? null,
    roles: row.roles ?? [],
    createdAtLabel: row.created_at_label ?? null,
  }))
}

type SettingsRoleRaw = {
  id: string
  code: string
  name: string
  account_type: 'internal' | 'external'
  description: string | null
  active: boolean
  authorities: string[] | null
  user_count: number
}

export async function getSettingsRoles(): Promise<SettingsRole[]> {
  const rows = await sql`
    select
      r.id,
      r.code,
      r.name,
      r.account_type,
      r.description,
      r.active,
      coalesce(
        (
          select jsonb_agg(a.code order by a.code)
          from role_authority ra
          join authority a
            on a.id = ra.authority_id
          where ra.role_id = r.id
        ),
        '[]'::jsonb
      ) as authorities,
      (select count(*)::int from app_user_role aur where aur.role_id = r.id) as user_count
    from role r
    order by r.account_type, r.code
  `

  return (rows as SettingsRoleRaw[]).map((row) => ({
    id: row.id,
    code: row.code,
    name: row.name,
    accountType: row.account_type,
    description: row.description ?? null,
    active: row.active,
    authorities: row.authorities ?? [],
    userCount: row.user_count,
  }))
}

type SettingsAuthorityRaw = {
  id: string
  code: string
  name: string
  description: string | null
  roles: string[] | null
}

export async function getSettingsAuthorities(): Promise<SettingsAuthority[]> {
  const rows = await sql`
    select
      a.id,
      a.code,
      a.name,
      a.description,
      coalesce(
        (
          select jsonb_agg(r.code order by r.code)
          from role_authority ra
          join role r
            on r.id = ra.role_id
          where ra.authority_id = a.id
        ),
        '[]'::jsonb
      ) as roles
    from authority a
    order by a.code
  `

  return (rows as SettingsAuthorityRaw[]).map((row) => ({
    id: row.id,
    code: row.code,
    name: row.name,
    description: row.description ?? null,
    roles: row.roles ?? [],
  }))
}
