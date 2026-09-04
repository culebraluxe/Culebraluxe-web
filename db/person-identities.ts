import { sql, db, DbFailureError } from './client'

import type { IdentityMatch, NormalizedIdentityHint } from '../lib/crm-intake-types'
import type { AtomicPersonClaimInput, IdentityOwnership } from '../lib/crm-person-types'
import type { QueryExecutor, QueryRow, TransactionExecutor } from './query-executor'

export function semanticPhoneKey(value: string): string {
  const digits = value.replace(/\D/g, '')
  return digits.length === 11 && digits.startsWith('1') ? digits.slice(1) : digits
}

const runNeonTransaction: TransactionExecutor = async (buildQueries) => {
  const result = await db.transaction('person-identities.tx', async (tx) =>
    Promise.all(buildQueries(tx as QueryExecutor)),
  )
  if (!result.ok) throw new DbFailureError(result.error)
  return result.data as QueryRow[][]
}

export async function personExists(personId: string, execute: QueryExecutor = sql) {
  const rows = await execute`
    select id from person
    where id = ${personId} and archived_at is null
    limit 1
  `
  return rows.length > 0
}

function lookupValues(hint: NormalizedIdentityHint) {
  return {
    identityType: hint.kind === 'external' ? 'external' : hint.kind,
    normalizedIdentityValue: hint.kind === 'phone' ? semanticPhoneKey(hint.normalizedValue) : hint.normalizedValue,
    sourceSystem: hint.kind === 'external' ? hint.sourceSystem : null,
  }
}

export async function findIdentityMatches(
  hint: NormalizedIdentityHint,
  execute: QueryExecutor = sql,
): Promise<IdentityMatch[]> {
  const { identityType, normalizedIdentityValue, sourceSystem } = lookupValues(hint)
  const rows = await execute`
    with lookup as (
      select
        ${identityType}::text as identity_type,
        ${normalizedIdentityValue}::text as identity_value,
        ${normalizedIdentityValue}::text as semantic_value,
        ${sourceSystem}::text as source_system
    )
    select pi.id as identity_id, pi.person_id, pi.identity_value
    from person_identity pi
    join person p on p.id = pi.person_id
    cross join lookup l
    where pi.identity_type = l.identity_type
      and (
        (l.identity_type = 'phone'
          and (case
            when length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) = 11
              and left(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 1) = '1'
            then substring(regexp_replace(pi.identity_value, '[^0-9]', '', 'g') from 2)
            else regexp_replace(pi.identity_value, '[^0-9]', '', 'g')
          end) = l.semantic_value)
        or (l.identity_type = 'email'
          and lower(trim(pi.identity_value)) = lower(trim(l.semantic_value)))
        or (l.identity_type <> 'phone' and l.identity_type <> 'email'
          and pi.identity_value = l.semantic_value)
      )
      and (l.source_system is null or pi.source_system = l.source_system)
      and p.archived_at is null
    order by pi.person_id, pi.id
  `
  return (rows as { identity_id: string; person_id: string; identity_value: string }[]).map((row) => ({
    identityId: row.identity_id,
    personId: row.person_id,
    kind: hint.kind,
    normalizedValue: row.identity_value,
  }))
}

export async function findIdentityMatch(
  hint: NormalizedIdentityHint,
  execute: QueryExecutor = sql,
): Promise<IdentityMatch | null> {
  const matches = await findIdentityMatches(hint, execute)
  if (matches.length > 1) throw new Error(`ambiguous canonical identity ownership: ${hint.kind}:${hint.normalizedValue}`)
  return matches[0] ?? null
}

export async function findIdentityOwnerships(
  hint: NormalizedIdentityHint,
  execute: QueryExecutor = sql,
): Promise<IdentityOwnership[]> {
  const { identityType, normalizedIdentityValue, sourceSystem } = lookupValues(hint)
  const rows = await execute`
    with lookup as (
      select
        ${identityType}::text as identity_type,
        ${normalizedIdentityValue}::text as identity_value,
        ${normalizedIdentityValue}::text as semantic_value,
        ${sourceSystem}::text as source_system
    )
    select pi.id as identity_id, pi.person_id, pi.identity_value, p.archived_at
    from person_identity pi
    join person p on p.id = pi.person_id
    cross join lookup l
    where pi.identity_type = l.identity_type
      and (
        (l.identity_type = 'phone'
          and (case
            when length(regexp_replace(pi.identity_value, '[^0-9]', '', 'g')) = 11
              and left(regexp_replace(pi.identity_value, '[^0-9]', '', 'g'), 1) = '1'
            then substring(regexp_replace(pi.identity_value, '[^0-9]', '', 'g') from 2)
            else regexp_replace(pi.identity_value, '[^0-9]', '', 'g')
          end) = l.semantic_value)
        or (l.identity_type = 'email'
          and lower(trim(pi.identity_value)) = lower(trim(l.semantic_value)))
        or (l.identity_type <> 'phone' and l.identity_type <> 'email'
          and pi.identity_value = l.semantic_value)
      )
      and (l.source_system is null or pi.source_system = l.source_system)
    order by pi.person_id, pi.id
  `
  return (rows as { identity_id: string; person_id: string; identity_value: string; archived_at: string | null }[]).map((row) => ({
    identityId: row.identity_id,
    personId: row.person_id,
    kind: hint.kind,
    normalizedValue: row.identity_value,
    archived: row.archived_at !== null,
  }))
}

export async function findIdentityOwnership(
  hint: NormalizedIdentityHint,
  execute: QueryExecutor = sql,
): Promise<IdentityOwnership | null> {
  const ownerships = await findIdentityOwnerships(hint, execute)
  if (ownerships.length > 1) throw new Error(`ambiguous canonical identity ownership: ${hint.kind}:${hint.normalizedValue}`)
  return ownerships[0] ?? null
}

export async function createPersonWithIdentities(
  input: AtomicPersonClaimInput,
  executeTransaction: TransactionExecutor = runNeonTransaction,
) {
  if (input.identities.length === 0) throw new Error('At least one identity is required to create a person.')
  const identities = [...input.identities].sort((left, right) =>
    `${left.kind}:${left.normalizedValue}`.localeCompare(`${right.kind}:${right.normalizedValue}`),
  )
  await executeTransaction((execute) => [
    execute`
      insert into person (id, display_name, role, status)
      values (${input.personId}, ${input.displayName}, ${input.role}, 'new')
    `,
    ...identities.map((identity) => execute`
      insert into person_identity (
        person_id, identity_type, identity_value, source_system, is_primary
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
