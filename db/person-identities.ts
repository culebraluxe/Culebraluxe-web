import { sql, db, DbFailureError } from './client'

import type {
  IdentityMatch,
  NormalizedIdentityHint,
} from '../lib/crm-intake-types'
import type {
  AtomicPersonClaimInput,
  IdentityOwnership,
} from '../lib/crm-person-types'
import type {
  QueryExecutor,
  QueryRow,
  TransactionExecutor,
} from './query-executor'

const runNeonTransaction: TransactionExecutor = async (buildQueries) => {
  const result = await db.transaction('person-identities.tx', async (tx) =>
    Promise.all(buildQueries(tx as QueryExecutor)),
  )
  if (!result.ok) throw new DbFailureError(result.error)
  return result.data as QueryRow[][]
}

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

export async function findIdentityOwnership(
  hint: NormalizedIdentityHint,
  execute: QueryExecutor = sql,
): Promise<IdentityOwnership | null> {
  const identityType = hint.kind === 'external' ? 'external' : hint.kind
  const sourceSystem = hint.kind === 'external' ? hint.sourceSystem : null

  const rows = await execute`
    select
      pi.id as identity_id,
      pi.person_id,
      pi.identity_value,
      p.archived_at
    from person_identity pi
    join person p on p.id = pi.person_id
    where pi.identity_type = ${identityType}
      and pi.identity_value = ${hint.normalizedValue}
      and (${sourceSystem}::text is null or pi.source_system = ${sourceSystem})
    limit 1
  `

  const row = rows[0] as
    | {
        identity_id: string
        person_id: string
        identity_value: string
        archived_at: string | null
      }
    | undefined

  return row
    ? {
        identityId: row.identity_id,
        personId: row.person_id,
        kind: hint.kind,
        normalizedValue: row.identity_value,
        archived: row.archived_at !== null,
      }
    : null
}

export async function createPersonWithIdentities(
  input: AtomicPersonClaimInput,
  executeTransaction: TransactionExecutor = runNeonTransaction,
) {
  if (input.identities.length === 0) {
    throw new Error('At least one identity is required to create a person.')
  }

  const identities = [...input.identities].sort((left, right) => {
    const leftKey = `${left.kind}:${left.normalizedValue}`
    const rightKey = `${right.kind}:${right.normalizedValue}`
    return leftKey.localeCompare(rightKey)
  })

  await executeTransaction((execute) => [
    execute`
      insert into person (
        id,
        display_name,
        role,
        status
      ) values (
        ${input.personId},
        ${input.displayName},
        ${input.role},
        'new'
      )
    `,
    ...identities.map((identity) => execute`
      insert into person_identity (
        person_id,
        identity_type,
        identity_value,
        source_system,
        is_primary
      ) values (
        ${input.personId},
        ${identity.kind === 'external' ? 'external' : identity.kind},
        ${identity.normalizedValue},
        ${identity.sourceSystem ?? null},
        ${identity.isPrimary}
      )
    `),
  ])
}
