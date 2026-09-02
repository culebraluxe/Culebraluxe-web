import type { QueryExecutor } from '../db/query-executor'
import { semanticPhoneKey } from '../db/person-identities'
import {
  planSourceLinkedPersonConsolidations,
  type PersonConsolidationPlan,
  type PersonIdentitySet,
  type SourceProfileAnchor,
} from '../lib/relationship-intel/person-consolidation-plan'

type EnvTarget = 'prod'

type LoadedPlan = {
  plan: PersonConsolidationPlan
  profiles: SourceProfileAnchor[]
  people: PersonIdentitySet[]
  names: Map<string, string>
}

function flag(name: string): string | undefined {
  const i = process.argv.indexOf(name)
  return i >= 0 ? process.argv[i + 1] : undefined
}

function identityKey(type: string, value: string): string {
  const raw = value.trim()
  if (type === 'email') return `email:${raw.toLowerCase()}`
  if (type === 'phone') return `phone:${semanticPhoneKey(raw)}`
  // Unknown identity kinds deliberately cannot be a subset of an Apple
  // email/phone profile, so their owner is never auto-consolidated.
  return `unsupported:${type}:${raw}`
}

async function loadPlan(execute: QueryExecutor, source: string): Promise<LoadedPlan> {
  const sourceRows = (await execute`
    select
      lp.source,
      lp.source_account,
      lp.source_contact_id,
      sl.canonical_person_id,
      li.identity_type,
      coalesce(nullif(li.normalized_value, ''), li.identity_value) as identity_value
    from l_person lp
    join integration_source_person_link sl
      on sl.source = lp.source
     and sl.source_account = lp.source_account
     and sl.source_identity_key = lp.source_contact_id
    left join l_person_identity li on li.l_person_id = lp.id
    where lp.source = ${source}
    order by lp.source_account, lp.source_contact_id, li.ordinal, li.id
  `) as Array<{
    source: string
    source_account: string
    source_contact_id: string
    canonical_person_id: string
    identity_type: string | null
    identity_value: string | null
  }>

  const profileMap = new Map<string, SourceProfileAnchor>()
  for (const row of sourceRows) {
    const sourceProfileKey = `${row.source}\u0000${row.source_account}\u0000${row.source_contact_id}`
    let profile = profileMap.get(sourceProfileKey)
    if (!profile) {
      profile = {
        sourceProfileKey,
        survivorPersonId: row.canonical_person_id,
        identityKeys: [],
      }
      profileMap.set(sourceProfileKey, profile)
    }
    if (
      row.identity_value &&
      (row.identity_type === 'email' || row.identity_type === 'phone')
    ) {
      const key = identityKey(row.identity_type, row.identity_value)
      if (!key.endsWith(':') && !profile.identityKeys.includes(key)) profile.identityKeys.push(key)
    }
  }

  const identityRows = (await execute`
    select p.id as person_id, p.display_name, pi.identity_type, pi.identity_value
    from person p
    left join person_identity pi on pi.person_id = p.id
    where p.archived_at is null
    order by p.id, pi.id
  `) as Array<{
    person_id: string
    display_name: string
    identity_type: string | null
    identity_value: string | null
  }>

  const peopleMap = new Map<string, PersonIdentitySet>()
  const names = new Map<string, string>()
  for (const row of identityRows) {
    names.set(row.person_id, row.display_name)
    let person = peopleMap.get(row.person_id)
    if (!person) {
      person = { personId: row.person_id, identityKeys: [] }
      peopleMap.set(row.person_id, person)
    }
    if (row.identity_type && row.identity_value) {
      const key = identityKey(row.identity_type, row.identity_value)
      if (!person.identityKeys.includes(key)) person.identityKeys.push(key)
    }
  }

  const profiles = [...profileMap.values()]
  const people = [...peopleMap.values()]
  return {
    profiles,
    people,
    names,
    plan: planSourceLinkedPersonConsolidations(profiles, people),
  }
}

async function mergeIdentityChildren(
  execute: QueryExecutor,
  survivorPersonId: string,
  loserPersonId: string,
): Promise<{ moved: number; deduped: number }> {
  const rows = (await execute`
    select id, person_id, identity_type, identity_value
    from person_identity
    where person_id in (${survivorPersonId}, ${loserPersonId})
    order by person_id, id
    for update
  `) as Array<{
    id: string
    person_id: string
    identity_type: string
    identity_value: string
  }>

  const survivorKeys = new Set(
    rows
      .filter((row) => row.person_id === survivorPersonId)
      .map((row) => identityKey(row.identity_type, row.identity_value)),
  )

  let moved = 0
  let deduped = 0
  for (const row of rows.filter((candidate) => candidate.person_id === loserPersonId)) {
    const key = identityKey(row.identity_type, row.identity_value)
    if (survivorKeys.has(key)) {
      await execute`delete from person_identity where id = ${row.id}`
      deduped += 1
      continue
    }
    await execute`
      update person_identity
      set person_id = ${survivorPersonId}, updated_at = now()
      where id = ${row.id}
    `
    survivorKeys.add(key)
    moved += 1
  }
  return { moved, deduped }
}

async function main() {
  const env = (flag('--env') ?? 'prod') as EnvTarget
  const source = flag('--source') ?? 'apple_contacts'
  const apply = process.argv.includes('--apply')
  if (env !== 'prod') throw new Error('Legacy Person split repair is PROD-only.')
  if (source !== 'apple_contacts') {
    throw new Error('Automatic consolidation is currently restricted to authoritative apple_contacts profiles.')
  }
  if (!process.env.DATABASE_URL_PROD) throw new Error('DATABASE_URL_PROD is not configured.')
  if (process.env.DATABASE_URL_DEV && process.env.DATABASE_URL_DEV === process.env.DATABASE_URL_PROD) {
    throw new Error('DATABASE_URL_PROD must not equal DATABASE_URL_DEV.')
  }
  if (apply && flag('--confirm') !== 'CONSOLIDATE_SOURCE_LINKED_PERSONS') {
    throw new Error('Apply requires --confirm CONSOLIDATE_SOURCE_LINKED_PERSONS.')
  }

  process.env.APP_ENV = 'production'
  const [{ db, sql, dbTargetInfo }, { refreshClientReadModels }] = await Promise.all([
    import('../db/client'),
    import('../db/client-read-models'),
  ])
  const databaseTarget = dbTargetInfo()
  if (databaseTarget.target !== 'prod') {
    throw new Error(`Refusing repair: resolved database target is ${databaseTarget.target}, not prod.`)
  }

  const before = await loadPlan(sql, source)
  const describe = (personId: string) => ({
    personId,
    displayName: before.names.get(personId) ?? null,
  })
  const preview = {
    source,
    eligibleConsolidations: before.plan.consolidations.map((item) => ({
      sourceProfileKey: item.sourceProfileKey,
      survivor: describe(item.survivorPersonId),
      loser: describe(item.loserPersonId),
    })),
    skippedMultiWinnerLosers: before.plan.skippedMultiWinnerLosers.map(describe),
    skippedPartialIdentityLosers: before.plan.skippedPartialIdentityLosers.map(describe),
  }

  if (!apply) {
    console.log(JSON.stringify({ env, mode: 'dry-run', databaseTarget, ...preview }, null, 2))
    return
  }

  if (before.plan.consolidations.length === 0) {
    console.log(JSON.stringify({ env, mode: 'applied', databaseTarget, ...preview, mergedPersons: 0 }, null, 2))
    console.log('[person-split-repair] SUCCESS: no eligible source-linked legacy Person splits.')
    return
  }

  const result = await db.transaction('source-linked-person-consolidation', async (tx) => {
    const current = await loadPlan(tx, source)
    const planned = current.plan.consolidations
    if (planned.length !== before.plan.consolidations.length) {
      throw new Error('Consolidation plan changed after transaction start; retry from a fresh dry-run.')
    }

    const personIds = [...new Set(planned.flatMap((item) => [item.survivorPersonId, item.loserPersonId]))]
    await tx`select id from person where id = any(${personIds}::uuid[]) for update`

    let identitiesMoved = 0
    let identitiesDeduped = 0
    await tx`create temporary table person_consolidation_map (loser uuid primary key, winner uuid not null) on commit drop`

    for (const item of planned) {
      const identityResult = await mergeIdentityChildren(tx, item.survivorPersonId, item.loserPersonId)
      identitiesMoved += identityResult.moved
      identitiesDeduped += identityResult.deduped
      await tx`
        insert into person_consolidation_map (loser, winner)
        values (${item.loserPersonId}, ${item.survivorPersonId})
      `
    }

    // Move every single-column FK child of Person. Unlike the old phone cleanup,
    // a uniqueness collision is NEVER swallowed: the whole transaction aborts
    // with the exact table/column so no half-merged Person can survive.
    await tx`
      do $merge$
      declare
        fk record;
        m record;
        remains boolean;
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
          for m in select loser, winner from person_consolidation_map loop
            begin
              execute format('update %s set %I = $1 where %I = $2', fk.table_name, fk.column_name, fk.column_name)
                using m.winner, m.loser;
            exception when unique_violation then
              raise exception using
                errcode = '23505',
                message = format(
                  'Person consolidation blocked by uniqueness constraint at %s.%s (loser=%s winner=%s)',
                  fk.table_name, fk.column_name, m.loser, m.winner
                );
            end;
          end loop;
        end loop;

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
          for m in select loser, winner from person_consolidation_map loop
            execute format('select exists(select 1 from %s where %I = $1)', fk.table_name, fk.column_name)
              into remains using m.loser;
            if remains then
              raise exception 'Person consolidation invariant failed: % still references loser %', fk.table_name, m.loser;
            end if;
          end loop;
        end loop;
      end
      $merge$
    `

    const loserIds = planned.map((item) => item.loserPersonId)
    const leftoverIdentities = await tx`
      select id from person_identity where person_id = any(${loserIds}::uuid[]) limit 1
    `
    if (leftoverIdentities.length > 0) throw new Error('Person consolidation invariant failed: loser still owns identity rows.')

    await tx`
      update person
      set archived_at = coalesce(archived_at, now()), updated_at = now()
      where id = any(${loserIds}::uuid[])
    `

    const after = await loadPlan(tx, source)
    const stillEligibleLosers = new Set(after.plan.consolidations.map((item) => item.loserPersonId))
    for (const loser of loserIds) {
      if (stillEligibleLosers.has(loser)) {
        throw new Error(`Person consolidation invariant failed: loser ${loser} remains eligible after merge.`)
      }
    }

    return {
      mergedPersons: planned.length,
      identitiesMoved,
      identitiesDeduped,
      remainingSkippedMultiWinner: after.plan.skippedMultiWinnerLosers.length,
      remainingSkippedPartialIdentity: after.plan.skippedPartialIdentityLosers.length,
    }
  })

  if (!result.ok) {
    throw new Error(`${result.error.kind}: ${result.error.detail ?? 'database transaction failed'} (incident ${result.error.incidentId})`)
  }

  await refreshClientReadModels()
  console.log(JSON.stringify({
    env,
    mode: 'applied',
    databaseTarget,
    ...preview,
    result: result.data,
    readModelsRefreshed: true,
  }, null, 2))
  console.log('[person-split-repair] SUCCESS: eligible source-linked legacy Person splits consolidated.')
}

main().catch((error: unknown) => {
  console.error('[person-split-repair] failed:', error instanceof Error ? error.message : String(error))
  process.exit(1)
})
