import { sql } from '@/legacy/db/client'
import type { EntitlementGrantProvider } from '@/legacy/services/entitlement/authorization-service'
import type { ServiceOperationKind } from '@/legacy/services/core'

/** Same role_entitlement source of truth used by the Rust security DAO.
 * Database failures propagate to BaseService error capture and deny the call. */
export class SqlRoleEntitlementProvider implements EntitlementGrantProvider {
  async hasGrant(appUserId: string, action: string, kind: ServiceOperationKind): Promise<boolean> {
    const rows = (await sql`
      select exists (
        select 1
        from app_user u
        join app_user_role aur on aur.app_user_id = u.id
        join security_role r on r.id = aur.role_id and r.active = true
        join role_entitlement re on re.role_id = r.id
        join entitlement e on e.id = re.entitlement_id and e.active = true
        where u.id = ${appUserId}::uuid and u.active = true
          and u.account_type = 'internal'
          and e.code = ${action} and e.operation_kind = ${kind}
      ) as allowed
    `) as unknown as Array<{ allowed: boolean }>
    return rows[0]?.allowed === true
  }
}
