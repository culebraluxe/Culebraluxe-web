import type { QueryExecutor } from '../db/query-executor'

type Owner = {
  identityId: string
  personId: string
  displayName: string
  identityValue: string
  isPrimary: boolean
  archived: boolean
  canonicalE164: string
}

type Group = {
  nanpKey: string
  identityCount: number
  personCount: number
  owners: Owner[]
}

function flag(name: string): string | undefined {
  const index = process.argv.indexOf(name)
  return index >= 0 ? process.argv[index + 1] : undefined
}

async function loadGroups(execute: QueryExecutor): Promise<Group[]> {
  const rows = await execute`
    with phone as (
      select
        pi.id as identity_id,
        pi.person_id,
        p.display_name,
        p.archived_at,
        pi.identity_value,
        pi.is_primary,
        regexp_replace(pi.identity_value, '[^0-9]', '', 'g') as digits
      from person_identity pi
      join person p on p.id = pi.person_id
      where pi.identity_type = 'phone'
    ),
    keyed as (
      select
        *,
        case
          when length(digits) = 10 then digits
          when length(digits) = 11 and left(digits, 1) = '1'
            then substring(digits from 2)
          else null
        end as nanp_key
      from phone
    )
    select
      nanp_key,
      count(*)::int as identity_count,
      count(distinct person_id)::int as person_count,
      json_agg(
        json_build_object(
          'identityId', identity_id,
          'personId', person_id,
          'displayName', display_name,
          'identityValue', identity_value,
          'isPrimary', is_primary,
          'archived', archived_at is not null,
          'canonicalE164', '+1' || nanp_key
        )
        order by archived_at nulls first, display_name, person_id, identity_id
      ) as owners
    from keyed
    where nanp_key is not null
    group by nanp_key
    order by nanp_key
  `

  return (rows as unknown as Array<{
    nanp_key: string
    identity_count: number
    person_count: number
    owners: Owner[]
  }>).map((row) => ({
    nanpKey: row.nanp_key,
    identityCount: Number(row.identity_count),
    personCount: Number(row.person_count),
    owners: row.owners,
  }))
}

function report(groups: Group[]) {
  const safe = groups.filter((group) => group.personCount === 1)
  const conflicts = groups.filter((group) => group.personCount > 1)
  const rowsToNormalize = safe.flatMap((group) =>
    group.owners.filter((owner) => owner.identityValue !== owner.canonicalE164),
  ).length
  const duplicateRowsToCollapse = safe.reduce(
    (count, group) => count + Math.max(0, group.identityCount - 1),
    0,
  )

  return {
    safeGroups: safe.length,
    rowsToNormalize,
    duplicateRowsToCollapse,
    crossPersonConflictGroups: conflicts.length,
    crossPersonConflicts: conflicts,
  }
}

async function main() {
  const environment = flag('--env') ?? 'prod'
  const apply = process.argv.includes('--apply')
  if (environment !== 'prod') {
    throw new Error('Phone identity cleanup is PROD-only.')
  }
  if (!process.env.DATABASE_URL_PROD) {
    throw new Error('DATABASE_URL_PROD is not configured in .env.local.')
  }
  if (
    process.env.DATABASE_URL_DEV &&
    process.env.DATABASE_URL_DEV === process.env.DATABASE_URL_PROD
  ) {
    throw new Error('DATABASE_URL_PROD must not equal DATABASE_URL_DEV.')
  }
  if (apply && flag('--confirm') !== 'NORMALIZE_NANP_PHONES') {
    throw new Error(
      'Apply requires --confirm NORMALIZE_NANP_PHONES. Dry-run made no changes.',
    )
  }

  process.env.APP_ENV = 'production'

  // Import the database only after APP_ENV is set. db/client constructs its
  // underlying Neon executor at module initialization, so a static import here
  // could silently bind this PROD-only command to DATABASE_URL_DEV.
  const [{ db, sql, dbTargetInfo }, { refreshClientReadModels }] =
    await Promise.all([
      import('../db/client'),
      import('../db/client-read-models'),
    ])
  const databaseTarget = dbTargetInfo()
  if (databaseTarget.target !== 'prod') {
    throw new Error(
      `Refusing cleanup: resolved database target is "${databaseTarget.target}", not "prod".`,
    )
  }

  const beforeGroups = await loadGroups(sql)
  const before = report(beforeGroups)

  if (!apply) {
    console.log(JSON.stringify({
      env: 'prod',
      mode: 'dry-run',
      databaseTarget,
      ...before,
    }, null, 2))
    console.log(
      '[identity-phone-cleanup] DRY RUN: no rows changed. Cross-Person conflicts are never auto-merged.',
    )
    return
  }

  const foreignKeys = await sql`
    select conrelid::regclass::text as referencing_table
    from pg_constraint
    where contype = 'f'
      and confrelid = 'person_identity'::regclass
    order by conrelid::regclass::text
  `
  if (foreignKeys.length > 0) {
    throw new Error(
      `person_identity has dependent foreign keys; cleanup aborted: ${foreignKeys
        .map((row) => String(row.referencing_table))
        .join(', ')}`,
    )
  }

  const result = await db.transaction(
    'identity-phone-cleanup',
    async (tx) => {
      await tx`
        select pi.id
        from person_identity pi
        where pi.identity_type = 'phone'
        for update
      `

      const groups = await loadGroups(tx)
      const safeGroups = groups.filter((group) => group.personCount === 1)
      let normalized = 0
      let collapsed = 0

      for (const group of safeGroups) {
        const canonical = `+1${group.nanpKey}`
        const ordered = [...group.owners].sort((left, right) => {
          const leftCanonical = left.identityValue === canonical ? 1 : 0
          const rightCanonical = right.identityValue === canonical ? 1 : 0
          if (leftCanonical !== rightCanonical) return rightCanonical - leftCanonical
          if (left.isPrimary !== right.isPrimary) return left.isPrimary ? -1 : 1
          return left.identityId.localeCompare(right.identityId)
        })
        const keeper = ordered[0]
        const duplicateIds = ordered.slice(1).map((owner) => owner.identityId)
        const preservePrimary = ordered.some((owner) => owner.isPrimary)

        if (duplicateIds.length > 0) {
          const deleted = await tx`
            delete from person_identity
            where id = any(${duplicateIds}::uuid[])
            returning id
          `
          collapsed += deleted.length
        }

        if (
          keeper.identityValue !== canonical ||
          (preservePrimary && !keeper.isPrimary)
        ) {
          const updated = await tx`
            update person_identity
            set identity_value = ${canonical},
                is_primary = ${preservePrimary},
                updated_at = now()
            where id = ${keeper.identityId}
            returning id
          `
          normalized += updated.length
        }
      }

      const remaining = report(await loadGroups(tx))
      if (
        remaining.rowsToNormalize !== 0 ||
        remaining.duplicateRowsToCollapse !== 0
      ) {
        throw new Error(
          `post-cleanup invariant failed: normalize=${remaining.rowsToNormalize} duplicate=${remaining.duplicateRowsToCollapse}`,
        )
      }

      return {
        normalized,
        collapsed,
        crossPersonConflictGroups: remaining.crossPersonConflictGroups,
        crossPersonConflicts: remaining.crossPersonConflicts,
      }
    },
  )

  if (!result.ok) {
    throw new Error(
      `${result.error.kind}: ${result.error.detail ?? 'database transaction failed'} ` +
        `(incident ${result.error.incidentId})`,
    )
  }

  await refreshClientReadModels()
  console.log(JSON.stringify({
    env: 'prod',
    mode: 'applied',
    databaseTarget,
    before: {
      safeGroups: before.safeGroups,
      rowsToNormalize: before.rowsToNormalize,
      duplicateRowsToCollapse: before.duplicateRowsToCollapse,
      crossPersonConflictGroups: before.crossPersonConflictGroups,
    },
    result: result.data,
    readModelsRefreshed: true,
  }, null, 2))
  console.log(
    '[identity-phone-cleanup] SUCCESS: safe same-Person NANP identities canonicalized; cross-Person conflicts unchanged.',
  )
}

main().catch((error: unknown) => {
  console.error('[identity-phone-cleanup] failed:', error instanceof Error ? error.message : String(error))
  process.exit(1)
})
