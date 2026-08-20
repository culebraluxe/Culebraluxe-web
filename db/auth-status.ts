import { sql } from './client'

// AUTH-02 read-only security status projection for the Settings area.
// Operational facts only — no tokens, no secrets, no provider credentials.
//
// AUTH-table counts are run conditionally after probing for the tables, because
// Postgres resolves table names at parse time.

export type SecurityStatus = {
  activeInternalUsers: number
  externalUsers: number
  usersWithNoRole: number
  usersWithMultipleRoles: number
  mappedAuthIdentities: number
  unmappedAppUsers: number
  ownerRoleAssignments: number
  inactiveUsersWithActiveRoleMappings: number
  accountTypeMismatchCount: number
}

export async function getSecurityStatus(): Promise<SecurityStatus> {
  const [userRows, authTableRows] = await Promise.all([
    sql`
      select
        (select count(*)::int from app_user where account_type = 'internal' and active = true) as active_internal_users,
        (select count(*)::int from app_user where account_type = 'external') as external_users
    `,
    sql`
      select
        to_regclass('app_user_role') is not null as has_roles,
        to_regclass('auth_identity') is not null as has_identities
    `,
  ])

  const userRow = userRows[0] as
    | { active_internal_users: number; external_users: number }
    | undefined
  const authTableRow = authTableRows[0] as
    | { has_roles: boolean; has_identities: boolean }
    | undefined
  const hasRoles = authTableRow?.has_roles ?? false
  const hasIdentities = authTableRow?.has_identities ?? false

  let usersWithNoRole = 0
  let usersWithMultipleRoles = 0
  let ownerRoleAssignments = 0
  let inactiveUsersWithActiveRoleMappings = 0
  let accountTypeMismatchCount = 0

  if (hasRoles) {
    const roleRows = await sql`
      select
        (
          select count(*)::int
          from app_user u
          where not exists (
            select 1 from app_user_role aur where aur.app_user_id = u.id
          )
        ) as users_with_no_role,
        (
          select count(*)::int
          from (
            select app_user_id
            from app_user_role
            group by app_user_id
            having count(*) > 1
          ) multi
        ) as users_with_multiple_roles,
        (
          select count(*)::int
          from app_user_role aur
          join role r on r.id = aur.role_id
          where r.code = 'owner'
        ) as owner_role_assignments,
        (
          select count(*)::int
          from app_user_role aur
          join app_user u on u.id = aur.app_user_id
          join role r on r.id = aur.role_id
          where u.active = false and r.active = true
        ) as inactive_users_with_active_role_mappings,
        (
          select count(*)::int
          from app_user_role aur
          join role r on r.id = aur.role_id
          join app_user u on u.id = aur.app_user_id
          where r.account_type <> u.account_type
        ) as account_type_mismatch_count
    `
    const roleRow = roleRows[0] as
      | {
          users_with_no_role: number
          users_with_multiple_roles: number
          owner_role_assignments: number
          inactive_users_with_active_role_mappings: number
          account_type_mismatch_count: number
        }
      | undefined
    usersWithNoRole = roleRow?.users_with_no_role ?? 0
    usersWithMultipleRoles = roleRow?.users_with_multiple_roles ?? 0
    ownerRoleAssignments = roleRow?.owner_role_assignments ?? 0
    inactiveUsersWithActiveRoleMappings =
      roleRow?.inactive_users_with_active_role_mappings ?? 0
    accountTypeMismatchCount = roleRow?.account_type_mismatch_count ?? 0
  }

  let mappedAuthIdentities = 0
  let unmappedAppUsers = userRow?.active_internal_users ?? 0
  if (hasIdentities) {
    const identityRows = await sql`
      select
        (select count(*)::int from auth_identity) as mapped_auth_identities,
        (
          select count(*)::int
          from app_user u
          where not exists (
            select 1 from auth_identity ai where ai.app_user_id = u.id
          )
        ) as unmapped_app_users
    `
    const identityRow = identityRows[0] as
      | { mapped_auth_identities: number; unmapped_app_users: number }
      | undefined
    mappedAuthIdentities = identityRow?.mapped_auth_identities ?? 0
    unmappedAppUsers = identityRow?.unmapped_app_users ?? 0
  }

  return {
    activeInternalUsers: userRow?.active_internal_users ?? 0,
    externalUsers: userRow?.external_users ?? 0,
    usersWithNoRole,
    usersWithMultipleRoles,
    mappedAuthIdentities,
    unmappedAppUsers,
    ownerRoleAssignments,
    inactiveUsersWithActiveRoleMappings,
    accountTypeMismatchCount,
  }
}
