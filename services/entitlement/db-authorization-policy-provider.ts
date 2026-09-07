import { sql } from '@/db/client'
import {
  DEFAULT_AUTHORIZATION_POLICIES,
  type AuthorizationPolicy,
  type AuthorizationPolicyProvider,
} from './authorization-service'
import type { SecurityLevel } from '../security/level'

const LEVELS: ReadonlySet<string> = new Set(['ROOT', 'BUSINESS_POWER_USER', 'USER', 'GUEST'])

type PolicyRow = {
  id: string
  domain: string | null
  action: string | null
  operation: string | null
  kind: string | null
  min_level: string
}

/**
 * DB-backed policy source (table `service_authorization_policy`).
 *
 * The in-code default rules are kept as a floor so an un-migrated or partial DB
 * can never silently widen access; DB rows override by id. A missing table
 * fails safe (floor only), never open.
 */
export class SqlAuthorizationPolicyProvider implements AuthorizationPolicyProvider {
  async policies(): Promise<readonly AuthorizationPolicy[]> {
    let rows: PolicyRow[] = []
    try {
      rows = (await sql`
        select id, domain, action, operation, kind, min_level
        from service_authorization_policy
      `) as unknown as PolicyRow[]
    } catch {
      rows = [] // table absent -> rely on the in-code floor
    }

    const byId = new Map<string, AuthorizationPolicy>()
    for (const rule of DEFAULT_AUTHORIZATION_POLICIES) byId.set(rule.id, rule)

    for (const row of rows) {
      if (!LEVELS.has(row.min_level)) continue
      const kind = row.kind as AuthorizationPolicy['kind'] | null
      if (kind !== null && kind !== 'query' && kind !== 'command') continue
      byId.set(row.id, {
        id: row.id,
        domain: row.domain ?? undefined,
        action: row.action ?? undefined,
        operation: row.operation ?? undefined,
        kind: kind ?? undefined,
        minLevel: row.min_level as SecurityLevel,
      })
    }
    return [...byId.values()]
  }
}
