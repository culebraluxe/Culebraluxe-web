import { sql } from './client'

import type {
  IdentityMatch,
  NormalizedIdentityHint,
} from '../lib/crm-intake-types'
import type { QueryExecutor } from './query-executor'

export async function personExists(
  personId: string,
  execute: QueryExecutor = sql,
) {
  const rows = await execute`
    select id
    from person
    where id = ${personId}
      and archived_at is null
    limit 1
  `

  return rows.length > 0
}

export async function findIdentityMatch(
  hint: NormalizedIdentityHint,
  execute: QueryExecutor = sql,
): Promise<IdentityMatch | null> {
  const identityType = hint.kind === 'external' ? 'external' : hint.kind
  const sourceSystem = hint.kind === 'external' ? hint.sourceSystem : null

  const rows = await execute`
    select
      pi.id as identity_id,
      pi.person_id,
      pi.identity_value
    from person_identity pi
    join person p on p.id = pi.person_id
    where pi.identity_type = ${identityType}
      and pi.identity_value = ${hint.normalizedValue}
      and (${sourceSystem}::text is null or pi.source_system = ${sourceSystem})
      and p.archived_at is null
    limit 1
  `

  const row = rows[0] as
    | {
        identity_id: string
        person_id: string
        identity_value: string
      }
    | undefined

  return row
    ? {
        identityId: row.identity_id,
        personId: row.person_id,
        kind: hint.kind,
        normalizedValue: row.identity_value,
      }
    : null
}
