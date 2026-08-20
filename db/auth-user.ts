import { sql } from './client'
import type { ActingUser } from '@/lib/auth/types'

// AUTH-02 canonical security read service.
//
// Resolves an app_user id to its effective security principal:
//   app_user → active roles → active role authorities
//
// - inactive app_user is NOT an acting principal (returns null)
// - inactive roles do not grant authorities
// - authority codes are deduplicated and deterministically ordered
// - no business-domain reads are mixed in here

type SecurityPrincipalRow = {
  app_user_id: string
  display_name: string
  email: string | null
  account_type: 'internal' | 'external'
  person_id: string | null
  role_codes: string[] | null
  authority_codes: string[] | null
}

export async function getSecurityPrincipal(
  appUserId: string,
): Promise<ActingUser | null> {
  const rows = await sql`
    select
      u.id as app_user_id,
      u.display_name,
      u.email,
      u.account_type,
      u.person_id,
      (
        select coalesce(jsonb_agg(code order by code), '[]'::jsonb)
        from (
          select distinct r.code
          from app_user_role aur
          join role r
            on r.id = aur.role_id
            and r.active = true
          where aur.app_user_id = u.id
        ) roles
      ) as role_codes,
      (
        select coalesce(jsonb_agg(code order by code), '[]'::jsonb)
        from (
          select distinct a.code
          from app_user_role aur
          join role r
            on r.id = aur.role_id
            and r.active = true
          join role_authority ra
            on ra.role_id = r.id
          join authority a
            on a.id = ra.authority_id
          where aur.app_user_id = u.id
        ) authorities
      ) as authority_codes
    from app_user u
    where u.id = ${appUserId}
      and u.active = true
    limit 1
  `

  const row = rows[0] as SecurityPrincipalRow | undefined
  if (!row) return null

  return {
    appUserId: row.app_user_id,
    displayName: row.display_name,
    email: row.email ?? null,
    accountType: row.account_type,
    roleCodes: row.role_codes ?? [],
    authorityCodes: row.authority_codes ?? [],
    personId: row.person_id ?? null,
  }
}
