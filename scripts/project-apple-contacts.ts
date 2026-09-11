#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Apple Contacts — l_person CURRENT-STATE relational-load projection (SUPPORT-2).
//
//   pnpm contacts:project:dev    # --env dev   (DATABASE_URL_DEV)
//   pnpm contacts:project:prod   # --env prod  (DATABASE_URL_PROD)
//
// l_person is the CURRENT relational/load projection — NOT the immutable ODS
// history. It represents ONLY the current successful Apple snapshot:
//
//   CURRENT SNAPSHOT (integration_source_snapshot_member)
//       -> latest staged revision per current member (even if that revision was
//          created by an older batch, e.g. an exact replay)
//       -> l_person current rows
//       -> prune l_person rows that are NOT members of the current snapshot
//
// ODS history (integration_staged_contact_profile / integration_inbox) is NEVER
// truncated or rewritten. This projection only rebuilds the current-state
// l_person / l_person_identity / l_person_address and NEVER mutates canonical
// person / person_identity.
//
// The rebuild is atomic in a single DB transaction so a failed projection cannot
// leave a half-current population.
// ---------------------------------------------------------------------------
import { Pool } from '@neondatabase/serverless'
import { fileURLToPath } from 'node:url'
import { resolve as resolvePath } from 'node:path'

const isMain = process.argv[1]
  ? fileURLToPath(import.meta.url) === resolvePath(process.argv[1])
  : false

function flag(name: string): string | undefined {
  const i = process.argv.indexOf(name)
  return i >= 0 && i + 1 < process.argv.length ? process.argv[i + 1] : undefined
}

const SOURCE = 'apple_contacts'

/**
 * Latest immutable staged revision per CURRENT-SNAPSHOT member.
 * `current_snapshot` = membership of the resolved latest LOADED batch. A current
 * member uses its LATEST staged revision even when that revision belongs to an
 * older batch (an exact replay produces no new staged row).
 * $1 = batch id, $2 = source, $3 = source_account.
 */
const LATEST_CTE = `
  with current_snapshot as (
    select m.source, m.source_account, m.source_identity_key
    from integration_source_snapshot_member m
    where m.integration_intake_batch_id = $1
  ),
  latest as (
    select distinct on (scp.source, scp.source_account, scp.source_contact_id)
      scp.id as staged_profile_id, scp.integration_intake_batch_id,
      scp.source, scp.source_account, scp.source_contact_id,
      scp.revision, scp.payload_fingerprint, scp.reconciliation_status,
      scp.candidate_person_id, scp.profile
    from integration_staged_contact_profile scp
    join current_snapshot cs
      on cs.source = scp.source
     and cs.source_account = scp.source_account
     and cs.source_identity_key = scp.source_contact_id
    where scp.source = $2
    order by scp.source, scp.source_account, scp.source_contact_id, scp.revision desc
  )
`

const L_PERSON_UPSERT_SQL = `
  ${LATEST_CTE}
  insert into l_person (
    integration_staged_contact_profile_id, integration_intake_batch_id,
    source, source_account, source_contact_id, source_revision, payload_fingerprint,
    display_name, name_prefix, given_name, middle_name, family_name, name_suffix,
    nickname, organization, department, job_title, note, phones, emails, display_address,
    reconciliation_status, candidate_person_id
  )
  select
    staged_profile_id, integration_intake_batch_id, source, source_account, source_contact_id,
    revision, payload_fingerprint,
    coalesce(
      -- Each name part must be NULLed when empty: Apple exports absent parts as
      -- EMPTY STRINGS, and concat_ws skips NULLs but NOT empty strings. Joining the
      -- raw values produced doubled spaces ("Juan A.  Santa Cruz", "Art  Buyer") in
      -- ~31% of l_person rows, which propagated to person.display_name and made the
      -- client search (a contiguous ILIKE over normalized-in-name search_text) return
      -- nothing for the human-typed name ("Maria Cruz" -> 0 while "Maria  Cruz" exists).
      nullif(trim(concat_ws(' ',
        nullif(trim(profile->'name'->>'prefix'), ''),
        nullif(trim(profile->'name'->>'given'), ''),
        nullif(trim(profile->'name'->>'middle'), ''),
        nullif(trim(profile->'name'->>'family'), ''),
        nullif(trim(profile->'name'->>'suffix'), ''))), ''),
      nullif(trim(profile->>'organization'), ''),
      nullif(trim(profile->'name'->>'nickname'), ''),
      source_contact_id,
      '(unnamed)'
    ),
    nullif(trim(profile->'name'->>'prefix'), ''),
    nullif(trim(profile->'name'->>'given'), ''),
    nullif(trim(profile->'name'->>'middle'), ''),
    nullif(trim(profile->'name'->>'family'), ''),
    nullif(trim(profile->'name'->>'suffix'), ''),
    nullif(trim(profile->'name'->>'nickname'), ''),
    nullif(trim(profile->>'organization'), ''),
    nullif(trim(profile->>'department'), ''),
    nullif(trim(profile->>'jobTitle'), ''),
    nullif(trim(profile->>'note'), ''),
    -- CORE INFO ON THE ROW: phones/emails live on l_person (no child table needed).
    -- Labels are kept — Home/Work matter, and they are what the legal/physical split
    -- (and the address fork into l_property) depends on.
    coalesce((
      select jsonb_agg(jsonb_build_object(
        'label', nullif(trim(x->>'label'), ''),
        'value', trim(x->>'value')))
      from jsonb_array_elements(coalesce(profile->'phones', '[]'::jsonb)) x
      where coalesce(trim(x->>'value'), '') <> ''
    ), '[]'::jsonb),
    coalesce((
      select jsonb_agg(jsonb_build_object(
        'label', nullif(trim(x->>'label'), ''),
        'value', trim(x->>'value')))
      from jsonb_array_elements(coalesce(profile->'emails', '[]'::jsonb)) x
      where coalesce(trim(x->>'value'), '') <> ''
    ), '[]'::jsonb),
    (
      -- CONVENTION (captain, 2026-09-10): in Apple Contacts there are only two address
      -- slots, and "Home" IS the legal address. Apple stores absent parts as empty
      -- strings, so each component is nullif(trim(...),'')-ed before concat_ws (which
      -- skips NULLs but NOT empty strings). Selection is LABEL-driven, not positional:
      -- the old postalAddresses[0].street picked whichever address happened to be first
      -- and kept only the street line, dropping city/state/ZIP/country.
      select nullif(trim(concat_ws(', ',
          nullif(trim(a->>'street'), ''),
          nullif(trim(a->>'city'), ''),
          nullif(trim(concat_ws(' ', nullif(trim(a->>'state'), ''), nullif(trim(a->>'postalCode'), ''))), ''),
          nullif(trim(a->>'country'), '')
        )), '')
      from jsonb_array_elements(profile->'postalAddresses') a
      order by case when a->>'label' = '_$!<Home>!$_' then 0 else 1 end
      limit 1
    ),
    reconciliation_status, candidate_person_id
  from latest
  on conflict (source, source_account, source_contact_id) do update set
    integration_staged_contact_profile_id = excluded.integration_staged_contact_profile_id,
    integration_intake_batch_id = excluded.integration_intake_batch_id,
    source_revision = excluded.source_revision,
    payload_fingerprint = excluded.payload_fingerprint,
    display_name = excluded.display_name,
    name_prefix = excluded.name_prefix,
    given_name = excluded.given_name,
    middle_name = excluded.middle_name,
    family_name = excluded.family_name,
    name_suffix = excluded.name_suffix,
    nickname = excluded.nickname,
    organization = excluded.organization,
    department = excluded.department,
    job_title = excluded.job_title,
    note = excluded.note,
    phones = excluded.phones,
    emails = excluded.emails,
    display_address = excluded.display_address,
    reconciliation_status = excluded.reconciliation_status,
    candidate_person_id = excluded.candidate_person_id
`

/** Prune l_person rows that are NOT members of the current snapshot. */
const PRUNE_SQL = `
  ${LATEST_CTE}
  delete from l_person lp
  where lp.source = $2
    and lp.source_account = $3
    and not exists (
      select 1 from current_snapshot cs
      where cs.source = lp.source
        and cs.source_account = lp.source_account
        and cs.source_identity_key = lp.source_contact_id
    )
`

const EMAILS_SQL = `
  ${LATEST_CTE}
  insert into l_person_identity (l_person_id, identity_type, identity_value, normalized_value, source_label, ordinal)
  select
    lp.id, 'email',
    trim(e.value->>'value'),
    trim(e.value->>'value'),
    nullif(trim(e.value->>'label'), ''),
    e.ordinal - 1
  from latest l
  join l_person lp
    on lp.source = l.source and lp.source_account = l.source_account
   and lp.source_contact_id = l.source_contact_id
  cross join lateral jsonb_array_elements(l.profile->'emails') with ordinality as e(value, ordinal)
  where trim(coalesce(e.value->>'value', '')) <> ''
  on conflict (l_person_id, identity_type, identity_value) do nothing
`

const PHONES_SQL = `
  ${LATEST_CTE}
  insert into l_person_identity (l_person_id, identity_type, identity_value, normalized_value, source_label, ordinal)
  select
    lp.id, 'phone',
    trim(e.value->>'value'),
    ('+' || regexp_replace(trim(e.value->>'value'), '[^0-9]', '', 'g')),
    nullif(trim(e.value->>'label'), ''),
    e.ordinal - 1
  from latest l
  join l_person lp
    on lp.source = l.source and lp.source_account = l.source_account
   and lp.source_contact_id = l.source_contact_id
  cross join lateral jsonb_array_elements(l.profile->'phones') with ordinality as e(value, ordinal)
  where trim(coalesce(e.value->>'value', '')) <> ''
  on conflict (l_person_id, identity_type, identity_value) do nothing
`

const APPLE_ID_SQL = `
  ${LATEST_CTE}
  insert into l_person_identity (l_person_id, identity_type, identity_value, normalized_value, source_label, ordinal)
  select
    lp.id, 'apple_contact',
    l.source_contact_id,
    l.source_contact_id,
    'Apple contact identifier',
    0
  from latest l
  join l_person lp
    on lp.source = l.source and lp.source_account = l.source_account
   and lp.source_contact_id = l.source_contact_id
  on conflict (l_person_id, identity_type, identity_value) do nothing
`

const ADDRESSES_SQL = `
  ${LATEST_CTE}
  insert into l_person_address (
    l_person_id, source_label, street, city, state, postal_code, country,
    iso_country_code, ordinal
  )
  select
    lp.id,
    nullif(trim(a.value->>'label'), ''),
    trim(a.value->>'street'),
    trim(a.value->>'city'),
    trim(a.value->>'state'),
    trim(a.value->>'postalCode'),
    trim(a.value->>'country'),
    trim(a.value->>'isoCountryCode'),
    a.ordinal - 1
  from latest l
  join l_person lp
    on lp.source = l.source and lp.source_account = l.source_account
   and lp.source_contact_id = l.source_contact_id
  cross join lateral jsonb_array_elements(l.profile->'postalAddresses') with ordinality as a(value, ordinal)
  where trim(coalesce(a.value->>'street', a.value->>'city', '')) <> ''
`


const L_PROPERTY_SQL = `
  ${LATEST_CTE}
  insert into l_property (
    source_system, source_account, source_key, source_label, address_type, ordinal,
    address_line1, city, state_or_province, postal_code, country, iso_country_code, raw
  )
  select
    l.source,
    l.source_account,
    l.source_contact_id || ':' || (a.ordinal - 1),
    nullif(trim(a.value->>'label'), ''),
    -- THE FORK: Home is the LEGAL address, Work is the PHYSICAL property.
    -- Label first (it carries the operator's intent); ordinal as the fallback
    -- (first address -> LEGAL, second -> PHYSICAL) when a label is missing.
    case
      when coalesce(a.value->>'label', '') ilike '%home%' then 'LEGAL'
      when coalesce(a.value->>'label', '') ilike '%work%' then 'PHYSICAL'
      when (a.ordinal - 1) = 0 then 'LEGAL'
      when (a.ordinal - 1) = 1 then 'PHYSICAL'
      else 'OTHER'
    end,
    a.ordinal - 1,
    nullif(trim(a.value->>'street'), ''),
    nullif(trim(a.value->>'city'), ''),
    nullif(trim(a.value->>'state'), ''),
    nullif(trim(a.value->>'postalCode'), ''),
    nullif(trim(a.value->>'country'), ''),
    nullif(trim(a.value->>'isoCountryCode'), ''),
    a.value
  from latest l
  cross join lateral jsonb_array_elements(l.profile->'postalAddresses') with ordinality as a(value, ordinal)
  where trim(coalesce(a.value->>'street', a.value->>'city', '')) <> ''
  on conflict (coalesce(source_system, ''), coalesce(source_account, ''), coalesce(source_key, ''))
  do update set
    source_label = excluded.source_label,
    address_type = excluded.address_type,
    address_line1 = excluded.address_line1,
    city = excluded.city,
    state_or_province = excluded.state_or_province,
    postal_code = excluded.postal_code,
    country = excluded.country,
    iso_country_code = excluded.iso_country_code,
    raw = excluded.raw,
    ingested_at = now()
`


async function runMain() {
  const env = (flag('--env') ?? 'dev').toLowerCase()
  const url = env === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
  if (env !== 'dev' && env !== 'prod') {
    console.error('--env must be dev|prod')
    process.exit(2)
  }
  if (!url) {
    console.error(`No DATABASE_URL_${env.toUpperCase()} configured (fail closed).`)
    process.exit(2)
  }
  if (env === 'prod' && url === process.env.DATABASE_URL_DEV) {
    console.error('PROD projection selected but the configured connection is the DEV URL (fail closed)')
    process.exit(2)
  }

  const pool = new Pool({ connectionString: url, ssl: true })
  try {
    const accountRows = await pool.query(
      `select distinct source_account from integration_intake_batch where source = $1 and source_account <> '' order by source_account`,
      [SOURCE],
    )
    const sourceAccounts = accountRows.rows.map((r) => String(r.source_account))
    if (sourceAccounts.length !== 1) {
      console.error(
        `Expected exactly one existing ${SOURCE} source_account for projection; found ${sourceAccounts.length}. Set the source account explicitly.`,
      )
      process.exit(2)
    }
    const sourceAccount = sourceAccounts[0]

    // Latest SUCCESSFUL/LOADED Apple batch for this source+account.
    const batchRows = await pool.query(
      `select id from integration_intake_batch
        where source = $1 and source_account = $2 and load_status = 'loaded'
        order by received_at desc, created_at desc
        limit 1`,
      [SOURCE, sourceAccount],
    )
    const batchRow = batchRows.rows[0] as { id: string } | undefined
    if (!batchRow) {
      console.error(`No LOADED ${SOURCE} batch found for source_account ${sourceAccount}; cannot project current snapshot.`)
      process.exit(2)
    }
    const batchId = batchRow.id

    const before = await pool.query(
      `select count(distinct source_contact_id)::int as n from l_person where source = $1 and source_account = $2`,
      [SOURCE, sourceAccount],
    )
    const existingBefore = Number(before.rows[0]?.n ?? 0)

    const client = await pool.connect()
    try {
      await client.query('begin')
      await client.query(L_PERSON_UPSERT_SQL, [batchId, SOURCE])
      await client.query(
        `delete from l_person_identity where l_person_id in (select id from l_person where source = $1 and source_account = $2)`,
        [SOURCE, sourceAccount],
      )
      await client.query(
        `delete from l_person_address where l_person_id in (select id from l_person where source = $1 and source_account = $2)`,
        [SOURCE, sourceAccount],
      )
      await client.query(PRUNE_SQL, [batchId, SOURCE, sourceAccount])
      await client.query(EMAILS_SQL, [batchId, SOURCE])
      await client.query(PHONES_SQL, [batchId, SOURCE])
      await client.query(APPLE_ID_SQL, [batchId, SOURCE])
      await client.query(ADDRESSES_SQL, [batchId, SOURCE])
      // DUAL-WRITE (ODS restructure step 2): the same addresses also land in l_property,
      // where they fork by label — Home is the legal address, Work is the physical
      // property. l_person_address stays populated until the readers are repointed
      // (step 3) and the promotion runs (step 4); only then is it dropped (step 5).
      await client.query(L_PROPERTY_SQL, [batchId, SOURCE])
      await client.query('commit')
    } catch (err) {
      await client.query('rollback')
      throw err
    } finally {
      client.release()
    }

    const after = await pool.query(
      `select count(distinct source_contact_id)::int as n from l_person where source = $1 and source_account = $2`,
      [SOURCE, sourceAccount],
    )
    const currentCount = Number(after.rows[0]?.n ?? 0)

    const membershipRows = await pool.query(
      `select count(*)::int as n from integration_source_snapshot_member
        where integration_intake_batch_id = $1`,
      [batchId],
    )
    const membershipCount = Number(membershipRows.rows[0]?.n ?? 0)

    const idCounts = await pool.query(
      `select identity_type, count(*)::int as n from l_person_identity
        where l_person_id in (select id from l_person where source = $1 and source_account = $2)
        group by identity_type order by identity_type`,
      [SOURCE, sourceAccount],
    )
    const addressCount = await pool.query(
      `select count(*)::int as n from l_person_address
        where l_person_id in (select id from l_person where source = $1 and source_account = $2)`,
      [SOURCE, sourceAccount],
    )

    console.log(
      JSON.stringify(
        {
          env,
          source: SOURCE,
          batchId,
          snapshotMembership: membershipCount,
          totals: { before: existingBefore, after: currentCount, pruned: Math.max(0, existingBefore - currentCount), error: 0 },
          identities: Object.fromEntries(idCounts.rows.map((r) => [String(r.identity_type), Number(r.n)])),
          addresses: Number(addressCount.rows[0]?.n ?? 0),
        },
        null,
        2,
      ),
    )
  } finally {
    await pool.end()
  }
}

if (isMain) {
  runMain().catch((err) => {
    console.error((err as Error).message ?? String(err))
    process.exit(1)
  })
}

