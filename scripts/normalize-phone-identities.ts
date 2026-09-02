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
      select *,
        case
          when length(digits) = 10 then digits
          when length(digits) = 11 and left(digits, 1) = '1' then substring(digits from 2)
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

function chooseSurvivor(group: Group): string {
  const perPerson = new Map<string, Owner[]>()
  for (const owner of group.owners) {
    const current = perPerson.get(owner.personId) ?? []
    current.push(owner)
    perPerson.set(owner.personId, current)
  }

  return [...perPerson.entries()]
    .sort(([leftId, left], [rightId, right]) => {
      const leftActive = left.some((owner) => !owner.archived) ? 1 : 0
      const rightActive = right.some((owner) => !owner.archived) ? 1 : 0
      if (leftActive !== rightActive) return rightActive - leftActive

      const leftPrimary = left.some((owner) => owner.isPrimary) ? 1 : 0
      const rightPrimary = right.some((owner) => owner.isPrimary) ? 1 : 0
      if (leftPrimary !== rightPrimary) return rightPrimary - leftPrimary

      const leftCanonical = left.some((owner) => owner.identityValue === owner.canonicalE164) ? 1 : 0
      const rightCanonical = right.some((owner) => owner.identityValue === owner.canonicalE164) ? 1 : 0
      if (leftCanonical !== rightCanonical) return rightCanonical - leftCanonical

      return leftId.localeCompare(rightId)
    })[0][0]
}

async function main() {
  const environment = flag('--env') ?? 'prod'
  const apply = process.argv.includes('--apply')
  if (environment !== 'prod') throw new Error('Phone identity cleanup is PROD-only.')
  if (!process.env.DATABASE_URL_PROD) throw new Error('DATABASE_URL_PROD is not configured in .env.local.')
  if (process.env.DATABASE_URL_DEV && process.env.DATABASE_URL_DEV === process.env.DATABASE_URL_PROD) {
    throw new Error('DATABASE_URL_PROD must not equal DATABASE_URL_DEV.')
  }
  if (apply && flag('--confirm') !== 'NORMALIZE_NANP_PHONES') {
    throw new Error('Apply requires --confirm NORMALIZE_NANP_PHONES.')
  }

  process.env.APP_ENV = 'production'

  const [{ db, sql, dbTargetInfo }, { refreshClientReadModels }] = await Promise.all([
    import('../db/client'),
    import('../db/client-read-models'),
  ])
  const databaseTarget = dbTargetInfo()
  if (databaseTarget.target !== 'prod') {
    throw new Error(`Refusing cleanup: resolved database target is "${databaseTarget.target}", not "prod".`)
  }

  const beforeGroups = await loadGroups(sql)
  const before = report(beforeGroups)

  if (!apply) {
    console.log(JSON.stringify({ env: 'prod', mode: 'dry-run', databaseTarget, ...before }, null, 2))
    return
  }

  const identityForeignKeys = await sql`
    select conrelid::regclass::text as referencing_table
    from pg_constraint
    where contype = 'f' and confrelid = 'person_identity'::regclass
    order by conrelid::regclass::text
  `
  if (identityForeignKeys.length > 0) {
    throw new Error(`person_identity has dependent foreign keys; cleanup aborted: ${identityForeignKeys.map((row) => String(row.referencing_table)).join(', ')}`)
  }

  const result = await db.transaction('identity-phone-cleanup', async (tx) => {
    await tx`select pi.id from person_identity pi where pi.identity_type = 'phone' for update`

    const groups = await loadGroups(tx)
    const conflicts = groups.filter((group) => group.personCount > 1)
    let mergedPersons = 0

    await tx`create temporary table if not exists person_phone_merge_map (loser uuid primary key, winner uuid not null) on commit drop`
    await tx`truncate person_phone_merge_map`

    for (const group of conflicts) {
      const winner = chooseSurvivor(group)
      const losers = [...new Set(group.owners.map((owner) => owner.personId))].filter((personId) => personId !== winner)
      const loserPhoneIds = group.owners.filter((owner) => owner.personId !== winner).map((owner) => owner.identityId)

      if (loserPhoneIds.length > 0) {
        await tx`delete from person_identity where id = any(${loserPhoneIds}::uuid[])`
      }

      for (const loser of losers) {
        await tx`insert into person_phone_merge_map (loser, winner) values (${loser}, ${winner}) on conflict (loser) do update set winner = excluded.winner`

        // Preserve canonical Person business state before archiving the loser.
        // Phone formatting is never allowed to erase a real buyer/seller role or
        // the richer profile fields accumulated on an older Person.
        await tx`
          update person survivor
          set
            role = case
              when survivor.role = loser.role then survivor.role
              when survivor.role = 'unclassified' then loser.role
              when loser.role = 'unclassified' then survivor.role
              when survivor.role = 'both' or loser.role = 'both' then 'both'
              when survivor.role in ('buyer', 'seller') and loser.role in ('buyer', 'seller') then 'both'
              else survivor.role
            end,
            location = coalesce(survivor.location, loser.location),
            budget_min = coalesce(survivor.budget_min, loser.budget_min),
            budget_max = coalesce(survivor.budget_max, loser.budget_max),
            preferred_areas = coalesce(survivor.preferred_areas, loser.preferred_areas),
            property_types = coalesce(survivor.property_types, loser.property_types),
            priorities = coalesce(survivor.priorities, loser.priorities),
            timeline = coalesce(survivor.timeline, loser.timeline),
            notes = coalesce(survivor.notes, loser.notes),
            assigned_user_id = coalesce(survivor.assigned_user_id, loser.assigned_user_id),
            updated_at = now()
          from person loser
          where survivor.id = ${winner}
            and loser.id = ${loser}
        `

        await tx`
          update person_identity
          set person_id = ${winner}, updated_at = now()
          where person_id = ${loser}
        `
        mergedPersons += 1
      }
    }

    if (mergedPersons > 0) {
      await tx`
        do $merge$
        declare
          fk record;
          m record;
        begin
          for fk in
            select
              c.conrelid::regclass::text as table_name,
              a.attname as column_name
            from pg_constraint c
            join lateral unnest(c.conkey) with ordinality as k(attnum, ord) on true
            join pg_attribute a on a.attrelid = c.conrelid and a.attnum = k.attnum
            where c.contype = 'f'
              and c.confrelid = 'person'::regclass
              and c.conrelid <> 'person_identity'::regclass
              and cardinality(c.conkey) = 1
          loop
            for m in select loser, winner from person_phone_merge_map loop
              begin
                execute format('update %s set %I = $1 where %I = $2', fk.table_name, fk.column_name, fk.column_name)
                  using m.winner, m.loser;
              exception when unique_violation then
                raise exception using
                  errcode = '23505',
                  message = format(
                    'Phone Person consolidation blocked by uniqueness constraint at %s.%s (loser=%s winner=%s)',
                    fk.table_name, fk.column_name, m.loser, m.winner
                  );
              end;
            end loop;
          end loop;
        end
        $merge$
      `

      await tx`
        update person p
        set archived_at = coalesce(p.archived_at, now()), updated_at = now()
        from person_phone_merge_map m
        where p.id = m.loser
      `
    }

    const postMergeGroups = await loadGroups(tx)
    let normalized = 0
    let collapsed = 0

    for (const group of postMergeGroups.filter((candidate) => candidate.personCount === 1)) {
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
        const deleted = await tx`delete from person_identity where id = any(${duplicateIds}::uuid[]) returning id`
        collapsed += deleted.length
      }

      if (keeper.identityValue !== canonical || (preservePrimary && !keeper.isPrimary)) {
        const updated = await tx`
          update person_identity
          set identity_value = ${canonical}, is_primary = ${preservePrimary}, updated_at = now()
          where id = ${keeper.identityId}
          returning id
        `
        normalized += updated.length
      }
    }

    const remaining = report(await loadGroups(tx))
    if (remaining.rowsToNormalize !== 0 || remaining.duplicateRowsToCollapse !== 0 || remaining.crossPersonConflictGroups !== 0) {
      throw new Error(`post-cleanup invariant failed: normalize=${remaining.rowsToNormalize} duplicate=${remaining.duplicateRowsToCollapse} crossPerson=${remaining.crossPersonConflictGroups}`)
    }

    return { normalized, collapsed, mergedPersons, crossPersonConflictGroups: 0 }
  })

  if (!result.ok) {
    throw new Error(`${result.error.kind}: ${result.error.detail ?? 'database transaction failed'} (incident ${result.error.incidentId})`)
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
  console.log('[identity-phone-cleanup] SUCCESS: NANP identities canonicalized and cross-Person phone collisions resolved.')
}

main().catch((error: unknown) => {
  console.error('[identity-phone-cleanup] failed:', error instanceof Error ? error.message : String(error))
  process.exit(1)
})
