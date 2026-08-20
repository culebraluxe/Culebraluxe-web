// AUTH-02 non-secret break-glass root readiness. Read-only; never exposes the
// secret, hash, OAuth secrets, or tokens.

import { sql } from '@/db/client'
import { getBreakGlassConfig } from './break-glass-config'

export type BreakGlassReadiness = {
  configured: boolean
  enabled: boolean
  rootResolvable: boolean
  rootActive: boolean
  ownerRolePresent: boolean
  auditTableAvailable: boolean
}

export async function getBreakGlassReadiness(): Promise<BreakGlassReadiness> {
  const config = getBreakGlassConfig()
  const configured = Boolean(config.appUserId && config.secretHash)

  const probe = await sql`
    select
      to_regclass('app_user_role') is not null as has_roles,
      to_regclass('security_audit_event') is not null as has_audit
  `
  const probeRow = probe[0] as
    | { has_roles: boolean; has_audit: boolean }
    | undefined
  const hasRoles = probeRow?.has_roles ?? false
  const hasAudit = probeRow?.has_audit ?? false

  let rootResolvable = false
  let rootActive = false
  let ownerRolePresent = false

  if (config.appUserId) {
    const userRows = await sql`
      select u.active
      from app_user u
      where u.id = ${config.appUserId}
      limit 1
    `
    const userRow = userRows[0] as { active: boolean } | undefined
    rootResolvable = userRow !== undefined
    rootActive = userRow?.active === true

    if (hasRoles) {
      const ownerRows = await sql`
        select 1
        from app_user_role aur
        join role r on r.id = aur.role_id
        where aur.app_user_id = ${config.appUserId}
          and r.code = 'owner'
        limit 1
      `
      ownerRolePresent = ownerRows.length > 0
    }
  }

  return {
    configured,
    enabled: config.enabled,
    rootResolvable,
    rootActive,
    ownerRolePresent,
    auditTableAvailable: hasAudit,
  }
}
