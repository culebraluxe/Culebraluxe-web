#!/usr/bin/env node
// ---------------------------------------------------------------------------
// Apple Contacts — l_person relational-load projection (SUPPORT-2).
//
//   pnpm contacts:project:dev    # --env dev   (DATABASE_URL_DEV)
//   pnpm contacts:project:prod   # --env prod  (DATABASE_URL_PROD)
//
// SET-BASED projection: ~7 SQL statements (INSERT ... SELECT from the immutable
// staged revisions) upsert the current-state l_person load rows and
// deterministically rebuild their l_person_identity + l_person_address
// children. No per-contact round-trips, so the 2,573-row load completes
// quickly. It is IDEMPOTENT and re-runnable:
//   - l_person is upserted by unique(source, source_account, source_contact_id)
//   - children are deterministically rebuilt (delete-then-insert for the source)
//   - replay produces zero duplicate load people / identities
// A later staged revision (changed) updates the existing l_person row.
// NEVER mutates canonical person / person_identity.
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

/** Latest immutable staged revision per (source, source_account, source_contact_id). */
const LATEST_CTE = `
  with latest as (
    select distinct on (scp.source, scp.source_account, scp.source_contact_id)
      scp.id as staged_profile_id, scp.integration_intake_batch_id,
      scp.source, scp.source_account, scp.source_contact_id,
      scp.revision, scp.payload_fingerprint, scp.reconciliation_status,
      scp.candidate_person_id, scp.profile
    from integration_staged_contact_profile scp
    order by scp.source, scp.source_account, scp.source_contact_id, scp.revision desc
  )
`

const L_PERSON_UPSERT_SQL = `
  ${LATEST_CTE}
  insert into l_person (
    integration_staged_contact_profile_id, integration_intake_batch_id,
    source, source_account, source_contact_id, source_revision, payload_fingerprint,
    display_name, name_prefix, given_name, middle_name, family_name, name_suffix,
    nickname, organization, department, job_title, display_address,
    reconciliation_status, candidate_person_id
  )
  select
    staged_profile_id, integration_intake_batch_id, source, source_account, source_contact_id,
    revision, payload_fingerprint,
    coalesce(
      nullif(trim(concat_ws(' ', profile->'name'->>'prefix', profile->'name'->>'given',
        profile->'name'->>'middle', profile->'name'->>'family', profile->'name'->>'suffix')), ''),
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
    (
      select nullif(trim(concat_ws(', ',
        p ->> 'street',
        nullif(trim(concat_ws(', ', p ->> 'city', nullif(trim(concat_ws(' ', p ->> 'state', p ->> 'postalCode')), ''))), ''),
        nullif(trim(p ->> 'country'), '')
      )), '')
      from jsonb_array_elements(profile->'postalAddresses') as p
      limit 1
    ),
    coalesce(reconciliation_status, 'unreviewed'),
    candidate_person_id
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
    display_address = excluded.display_address,
    reconciliation_status = excluded.reconciliation_status,
    candidate_person_id = excluded.candidate_person_id,
    updated_at = now()
`

const EMAILS_SQL = `
  ${LATEST_CTE}
  insert into l_person_identity (
    l_person_id, identity_type, identity_value, original_value, normalized_value,
    source_label, source_system, is_primary, ordinal
  )
  select
    lp.id, 'email',
    lower(trim(e.value->>'value')),
    trim(e.value->>'value'),
    lower(trim(e.value->>'value')),
    nullif(trim(e.value->>'label'), ''),
    '${SOURCE}', false,
    e.ordinal - 1
  from latest l
  join l_person lp
    on lp.source = l.source and lp.source_account = l.source_account
   and lp.source_contact_id = l.source_contact_id
  cross join lateral jsonb_array_elements(l.profile->'emails') with ordinality as e(value, ordinal)
  where trim(e.value->>'value') <> ''
  on conflict (l_person_id, identity_type, identity_value) do nothing
`

const PHONES_SQL = `
  ${LATEST_CTE}
  insert into l_person_identity (
    l_person_id, identity_type, identity_value, original_value, normalized_value,
    source_label, source_system, is_primary, ordinal
  )
  select
    lp.id, 'phone',
    '+' || regexp_replace(trim(e.value->>'value'), '\\D', '', 'g'),
    trim(e.value->>'value'),
    '+' || regexp_replace(trim(e.value->>'value'), '\\D', '', 'g'),
    nullif(trim(e.value->>'label'), ''),
    '${SOURCE}', false,
    e.ordinal - 1
  from latest l
  join l_person lp
    on lp.source = l.source and lp.source_account = l.source_account
   and lp.source_contact_id = l.source_contact_id
  cross join lateral jsonb_array_elements(l.profile->'phones') with ordinality as e(value, ordinal)
  where trim(e.value->>'value') <> ''
  on conflict (l_person_id, identity_type, identity_value) do nothing
`

const APPLE_ID_SQL = `
  insert into l_person_identity (
    l_person_id, identity_type, identity_value, original_value, normalized_value,
    source_label, source_system, is_primary, ordinal
  )
  select
    id, 'apple_contact', source_contact_id, source_contact_id, source_contact_id,
    null, '${SOURCE}', false, 0
  from l_person
  where source = '${SOURCE}' and source_contact_id <> ''
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
    const before = await pool.query(
      `select count(distinct source_contact_id)::int as n from l_person where source = $1`,
      [SOURCE],
    )
    const existingBefore = Number(before.rows[0]?.n ?? 0)

    await pool.query(L_PERSON_UPSERT_SQL)
    await pool.query('delete from l_person_identity where l_person_id in (select id from l_person where source = $1)', [SOURCE])
    await pool.query('delete from l_person_address where l_person_id in (select id from l_person where source = $1)', [SOURCE])
    await pool.query(EMAILS_SQL)
    await pool.query(PHONES_SQL)
    await pool.query(APPLE_ID_SQL)
    await pool.query(ADDRESSES_SQL)

    const inputRows = await pool.query(
      `select count(distinct (source, source_account, source_contact_id))::int as n
         from integration_staged_contact_profile where source = $1`,
      [SOURCE],
    )
    const input = Number(inputRows.rows[0]?.n ?? 0)
    const created = Math.max(0, input - existingBefore)
    const updated = existingBefore

    const idCounts = await pool.query(
      `select identity_type, count(*)::int as n from l_person_identity group by identity_type order by identity_type`,
    )
    const addressCount = await pool.query(`select count(*)::int as n from l_person_address`)

    const balanced = input === created + updated
    console.log(
      JSON.stringify(
        {
          env,
          source: SOURCE,
          totals: { input, projected_new: created, updated, error: 0 },
          identities: Object.fromEntries(idCounts.rows.map((r) => [String(r.identity_type), Number(r.n)])),
          addresses: Number(addressCount.rows[0]?.n ?? 0),
          balanced,
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

