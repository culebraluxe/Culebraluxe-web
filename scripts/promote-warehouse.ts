#!/usr/bin/env node
// ---------------------------------------------------------------------------
// THE PROMOTION — L tables -> Warehouse.  The only script that reads L tables.
//
//   Apple -> l_person (name, phones, emails, note)
//         -> l_property (addresses, typed LEGAL / PHYSICAL)
//   ...promote...
//   person / property / person_property  -> view
//
// It does what five old code paths did, in one place:
//   * identity resolution + Person creation  (was db/person-mastering.ts)
//   * display-name promotion                 (was db/enrich-people.ts)
//   * fact promotion (phones/emails/note)    (was scripts/promote-l-person-facts.ts)
//   * address -> Property + person_property  (was a half-built path that only ran once)
//
// IDENTITY (R3): phone/email, normalized. That is how a landing row finds its Person.
// NEWER WINS (R2): the landing row is the latest source state, so it wins.
// TYPES: LEGAL  -> person_property.relation_type = 'legal_address'
//        PHYSICAL -> person_property.relation_type = 'physical_property'
//        (the two names the listing forms already read)
//
// Idempotent. Dry-run by default; --apply writes.
//   node --env-file=.env.local --import tsx scripts/promote-warehouse.ts --env prod --apply
// ---------------------------------------------------------------------------
import { randomUUID } from 'node:crypto'
import { forgeDb, forgeDbTargetForUrl } from '../db/forge-db'

const argv = process.argv.slice(2)
const flag = (n: string) => {
  const i = argv.indexOf(n)
  return i >= 0 && i + 1 < argv.length ? argv[i + 1] : undefined
}
const ENV = (flag('--env') ?? 'dev').toLowerCase()
const APPLY = argv.includes('--apply')

if (ENV !== 'dev' && ENV !== 'prod') {
  console.error(`--env must be dev or prod (got ${ENV})`)
  process.exit(2)
}
if (ENV === 'prod') {
  if (!process.env.DATABASE_URL_PROD) {
    console.error('No DATABASE_URL_PROD configured (fail closed)')
    process.exit(2)
  }
  if (process.env.DATABASE_URL_PROD === process.env.DATABASE_URL_DEV) {
    console.error('PROD selected but connection is the DEV URL (fail closed)')
    process.exit(2)
  }
  // db/client resolves its target lazily, so this must be set before any import
  // that touches the database.
  process.env.APP_ENV = 'production'
}
const url = ENV === 'prod' ? process.env.DATABASE_URL_PROD : process.env.DATABASE_URL_DEV
if (!url) {
  console.error(`No DATABASE_URL_${ENV.toUpperCase()} configured (fail closed)`)
  process.exit(2)
}
const pool = forgeDb.forTarget(forgeDbTargetForUrl(url))

type LandingPerson = {
  id: string
  source_contact_id: string
  display_name: string | null
  note: string | null
  phones: Array<{ label: string | null; value: string }> | null
  emails: Array<{ label: string | null; value: string }> | null
  /** Apple Contacts Company/Organization — the owning entity. */
  organization: string | null
  legal_address: string | null
}


/** Landing rows: the person's core info + their LEGAL address, in one pass. */
const LANDING_SQL = `
  select
    lp.id,
    lp.source_contact_id,
    lp.display_name,
    lp.note,
    lp.phones,
    lp.emails,
    lp.organization,
    (select nullif(trim(concat_ws(', ',
        nullif(trim(p.address_line1), ''),
        nullif(trim(p.city), ''),
        nullif(trim(concat_ws(' ', nullif(trim(p.state_or_province), ''), nullif(trim(p.postal_code), ''))), '')
      )), '')
      from l_property p
      where p.source_system = lp.source
        and p.source_account = lp.source_account
        and p.source_key like lp.source_contact_id || ':%'
        and p.address_type = 'LEGAL'
      limit 1) as legal_address
  from l_person lp
  where lp.source = 'apple_contacts'
`

type LandingProperty = {
  source_key: string
  address_type: string | null
  address_line1: string | null
  city: string | null
  state_or_province: string | null
  postal_code: string | null
  country: string | null
  iso_country_code: string | null
}

const LANDING_PROPERTY_SQL = `
  select source_key, address_type, address_line1, city, state_or_province, postal_code,
         country, iso_country_code
    from l_property
   where source_system = 'apple_contacts'
`

const compose = (parts: Array<string | null>) =>
  parts.map((p) => (p ?? '').trim()).filter((p) => p.length > 0).join(', ')

type PropStats = {
  propertiesCreated: number
  propertiesUpdated: number
  propertiesLinked: number
  legacyAddressLinksRetired: number
}

/** l_property -> property + person_property (LEGAL->legal_address, PHYSICAL->physical_property). */
async function promotePropertyFacts(personByContact: Map<string, string>, stats: PropStats) {
  const landing = await pool.query<LandingProperty>(LANDING_PROPERTY_SQL)
  if (landing.rowCount === 0) return

  const rows = landing.rows
    .map((p) => {
      const personId = personByContact.get(p.source_key.split(':')[0])
      if (!personId) return null
      return {
        personId,
        key: `apple_contacts:${p.source_key}`, // the existing convention for these rows
        rel: (p.address_type ?? '').toUpperCase() === 'PHYSICAL' ? 'physical_property' : 'legal_address',
        line1: p.address_line1,
        city: p.city,
        state: p.state_or_province,
        postal: p.postal_code,
        country: p.country,
        // Apple's own country code is unreliable/absent; every address we ingest is USA.
        iso: 'us',
        location:
          compose([p.address_line1, p.city, compose([p.state_or_province, p.postal_code])]) || null,
      }
    })
    .filter((r): r is NonNullable<typeof r> => r !== null)

  if (rows.length === 0) return

  const keys = rows.map((r) => r.key)
  const existing = await pool.query<{ id: string; source_listing_key: string }>(
    `select id, source_listing_key from property
      where source_type = 'apple_contacts' and source_listing_key = any($1::text[])`,
    [keys],
  )
  const known = new Set(existing.rows.map((r) => r.source_listing_key))
  const fresh = rows.filter((r) => !known.has(r.key))
  const seen = rows.filter((r) => known.has(r.key))

  if (!APPLY) {
    stats.propertiesCreated = fresh.length
    stats.propertiesUpdated = seen.length
    stats.propertiesLinked = rows.length
    return
  }

  const cols = (subset: typeof rows) => [
    subset.map((r) => r.key),
    subset.map((r) => r.line1),
    subset.map((r) => r.city),
    subset.map((r) => r.state),
    subset.map((r) => r.postal),
    subset.map((r) => r.country),
    subset.map((r) => r.iso),
    subset.map((r) => r.location),
  ]

  if (fresh.length > 0) {
    // An address is a place, not a listing: no invented name, status stays 'prospect'.
    await pool.query(
      `insert into property (
         id, name, status, featured, source_type, source_provider, source_listing_key,
         source_modified_at, last_synced_at, address_line1, city, state_or_province,
         postal_code, country, iso_country_code, location
       )
       select gen_random_uuid(), null, 'prospect', false, 'apple_contacts', 'apple_contacts',
              u.key, now(), now(), u.line1, u.city, u.state, u.postal, u.country, u.iso, u.loc
         from unnest($1::text[], $2::text[], $3::text[], $4::text[], $5::text[],
                     $6::text[], $7::text[], $8::text[])
              as u(key, line1, city, state, postal, country, iso, loc)`,
      cols(fresh),
    )
    stats.propertiesCreated = fresh.length
  }

  if (seen.length > 0) {
    // Newer wins: the landing row is the latest source state for this address.
    await pool.query(
      `update property p set
         address_line1 = coalesce(nullif(trim(u.line1), ''), p.address_line1),
         city = coalesce(nullif(trim(u.city), ''), p.city),
         state_or_province = coalesce(nullif(trim(u.state), ''), p.state_or_province),
         postal_code = coalesce(nullif(trim(u.postal), ''), p.postal_code),
         country = coalesce(nullif(trim(u.country), ''), p.country),
         iso_country_code = coalesce(nullif(trim(u.iso), ''), p.iso_country_code),
         location = coalesce(nullif(trim(u.loc), ''), p.location),
         last_synced_at = now(),
         updated_at = now()
       from unnest($1::text[], $2::text[], $3::text[], $4::text[], $5::text[],
                   $6::text[], $7::text[], $8::text[])
            as u(key, line1, city, state, postal, country, iso, loc)
       where p.source_type = 'apple_contacts' and p.source_listing_key = u.key`,
      cols(seen),
    )
    stats.propertiesUpdated = seen.length
  }

  // The relationship, with the two names the listing forms read.
  const linked = await pool.query(
    `insert into person_property (person_id, property_id, relation_type, source_type, source_key)
     select x.person_id, p.id, x.rel, 'apple_contacts', x.key
       from unnest($1::uuid[], $2::text[], $3::text[]) as x(person_id, key, rel)
       join property p on p.source_type = 'apple_contacts' and p.source_listing_key = x.key
     on conflict (person_id, property_id, relation_type) do nothing
     returning id`,
    [rows.map((r) => r.personId), keys, rows.map((r) => r.rel)],
  )
  stats.propertiesLinked = linked.rowCount ?? 0

  // The earlier half-built path wrote the generic 'address' type, which the forms
  // never read. Now that the canonical link exists, retire the duplicate.
  const retired = await pool.query(
    `delete from person_property
      where relation_type = 'address' and source_key = any($1::text[])
      returning id`,
    [keys],
  )
  stats.legacyAddressLinksRetired = retired.rowCount ?? 0
}

async function main() {
  const { normalizePhone, normalizeEmail } = await import('../lib/relationship-intel/normalize')
  const { createPersonWithIdentities } = await import('../db/person-identities')

  console.log(`\nPROMOTE L -> WAREHOUSE (env=${ENV}, apply=${APPLY})`)

  const landing = await pool.query<LandingPerson>(LANDING_SQL)
  console.log(`  landing person rows: ${landing.rowCount}`)

  const stats = {
    matchedExisting: 0,
    created: 0,
    ambiguous: 0,
    factsUpdated: 0,
    identitiesAdded: 0,
    skippedNoIdentity: 0,
    propertiesCreated: 0,
    propertiesUpdated: 0,
    propertiesLinked: 0,
    legacyAddressLinksRetired: 0,
  }

  // ONE read of everything that currently owns an identity. No per-row queries.
  const owned = await pool.query<{ identity_type: string; identity_value: string; person_id: string }>(
    `select identity_type, identity_value, person_id from person_identity`,
  )
  const owners = new Map<string, Set<string>>()
  for (const r of owned.rows) {
    const key = `${r.identity_type}:${r.identity_value}`
    const set = owners.get(key) ?? new Set<string>()
    set.add(r.person_id)
    owners.set(key, set)
  }

  type Plan = {
    row: LandingPerson
    identities: Array<{ kind: 'email' | 'phone'; value: string }>
    personId: string | null
  }
  const plans: Plan[] = []

  for (const row of landing.rows) {
    // R3 — identity is phone/email, normalized.
    const identities: Array<{ kind: 'email' | 'phone'; value: string }> = []
    for (const e of row.emails ?? []) {
      const n = normalizeEmail(e.value)
      if (n.ok) identities.push({ kind: 'email', value: n.value })
    }
    for (const p of row.phones ?? []) {
      const n = normalizePhone(p.value)
      if (n.ok) identities.push({ kind: 'phone', value: n.value })
    }
    if (identities.length === 0) {
      stats.skippedNoIdentity += 1
      continue
    }

    const found = new Set<string>()
    for (const i of identities) for (const id of owners.get(`${i.kind}:${i.value}`) ?? []) found.add(id)

    if (found.size === 1) {
      stats.matchedExisting += 1
      plans.push({ row, identities, personId: [...found][0] })
    } else if (found.size === 0) {
      stats.created += 1
      plans.push({ row, identities, personId: null }) // created below when applying
    } else {
      stats.ambiguous += 1 // conflicting owners -> needs-review, not this script
    }
  }

  if (APPLY) {
    // New people (few — usually single digits).
    for (const plan of plans.filter((p) => p.personId === null)) {
      const personId = randomUUID()
      plan.personId = personId
      await createPersonWithIdentities({
        personId,
        displayName: plan.row.display_name ?? '(unnamed)',
        role: 'unclassified',
        identities: plan.identities.map((i, index) => ({
          kind: i.kind,
          normalizedValue: i.value,
          sourceSystem: 'apple_contacts',
          isPrimary: index === 0,
        })),
      })
    }

    // R2/R4 — the landing row is the latest source state, so it wins. One statement.
    const ready = plans.filter((p): p is Plan & { personId: string } => p.personId !== null)
    await pool.query(
      `update person p set
         display_name = coalesce(nullif(trim(u.name), ''), p.display_name),
         display_name_source = case when nullif(trim(u.name), '') is not null
                                    then 'apple_contacts' else p.display_name_source end,
         location = coalesce(nullif(trim(u.loc), ''), p.location),
         notes = coalesce(nullif(trim(u.note), ''), p.notes),
         company = coalesce(nullif(trim(u.company), ''), p.company),
         updated_at = now()
       from unnest($1::uuid[], $2::text[], $3::text[], $4::text[], $5::text[]) as u(id, name, loc, note, company)
       where p.id = u.id`,
      [
        ready.map((p) => p.personId),
        ready.map((p) => p.row.display_name),
        ready.map((p) => p.row.legal_address),
        ready.map((p) => p.row.note),
        ready.map((p) => p.row.organization),
      ],
    )
    stats.factsUpdated = ready.length

    // Any identity not already on the row's person. One statement.
    const flat = ready.flatMap((p) => p.identities.map((i) => ({ id: p.personId, kind: i.kind, value: i.value })))
    const added = await pool.query(
      `insert into person_identity (person_id, identity_type, identity_value, source_system, is_primary)
       select x.id, x.kind, x.value, 'apple_contacts', false
       from unnest($1::uuid[], $2::text[], $3::text[]) as x(id, kind, value)
       where not exists (
         select 1 from person_identity y
          where y.person_id = x.id
            and y.identity_type = x.kind
            and y.identity_value = x.value)
       returning id`,
      [flat.map((f) => f.id), flat.map((f) => f.kind), flat.map((f) => f.value)],
    )
    stats.identitiesAdded = added.rowCount ?? 0
  }

  // Addresses: l_property -> property + person_property. Same resolution as above,
  // so an address can only ever land on the Person its identities own.
  const personByContact = new Map<string, string>()
  for (const plan of plans) {
    if (plan.personId) personByContact.set(plan.row.source_contact_id.split(':')[0], plan.personId)
  }
  await promotePropertyFacts(personByContact, stats)

  console.log('  ' + JSON.stringify(stats))
  if (APPLY) {
    const { refreshClientReadModels } = await import('../db/client-read-models')
    await refreshClientReadModels()
    console.log('  read models refreshed')
  }
  console.log(APPLY ? '\nAPPLIED' : '\nDRY RUN — re-run with --apply to write')
}

main()
  .then(() => pool.end())
  .catch(async (err) => {
    console.error(err)
    await pool.end().catch(() => {})
    process.exit(1)
  })
