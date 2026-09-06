import { db } from './client'
import type { ActingUser } from '@/lib/auth/types'

// AUTH-02 canonical security read service.
//
// Resolves an app_user id to its effective security principal:
//   app_user → active security roles → active role authorities
//
// Migration 118 renames the historical authorization table `role` to
// `security_role` and reclaims `role` for business/domain positions. During
// rollout DEV/PROD may temporarily be on either side of that rename, so this
// projection deliberately supports both physical schemas.
//
// - inactive app_user is NOT an acting principal (returns null)
// - inactive security roles do not grant authorities
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

async function usesSecurityRoleTable(): Promise<boolean> {
  const r = await db.queryOne<{ security_role: string | null }>`
    select to_regclass('public.security_role')::text as security_role
  `
  return Boolean(r.ok && r.data?.security_role)
}

export async function getSecurityPrincipal(
  appUserId: string,
): Promise<ActingUser | null> {
  const securityRole = await usesSecurityRoleTable()

  const r = securityRole
    ? await db.queryOne<SecurityPrincipalRow>`
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
              join security_role r
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
              join security_role r
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
    : await db.queryOne<SecurityPrincipalRow>`
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

  // DB-HARDEN-01B — authorization FAILS CLOSED. If the canonical security
  // projection cannot be resolved (DB unavailable / schema drift / timeout),
  // return null (no principal → no access granted). The gateway logs the
  // incident for observability; we never fabricate a principal on failure.
  if (!r.ok) return null
  const row = r.data
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
