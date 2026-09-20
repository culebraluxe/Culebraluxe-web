/**
 * MAP THE REGRID LANDING TABLE INTO THE WAREHOUSE (property + person).
 *
 *   node --import tsx --env-file=.env.local scripts/load-regrid-warehouse.ts [--target dev|prod] [--dry-run] [--limit N]
 *
 * SOURCE: "l_Regrid" (migration 199) — the 138-field Regrid parcel export for Culebra.
 * TARGET: `property` and `person`, with the two tables this warehouse already uses to record where an outside
 * identity came from (`integration_source_person_link`, `person_property`).
 *
 * THE RULES, reused from this repository rather than invented here:
 *
 *   1. REGRID ENRICHES, IT NEVER OVERWRITES HUMAN TRUTH. The doctrine is the existing one
 *      (`db/regrid-property-enrichment.ts:85`): every parent column is written `coalesce(parent, source)`, so a
 *      hand-entered value — the form instances in `document_form_instance` — is simply not blank and Regrid cannot
 *      reach it. The source's OWN record always lands in its `regrid_*` column, and the full row lands in
 *      `regrid_data` so a later mapping never needs another purchase of the data.
 *   2. ONE PROPERTY PER PARCEL, keyed by `regrid_ll_uuid` (unique index, migration 201). A row matching an
 *      existing property by Catastro number (`listing_identifier`) is enriched too; a row matching nothing becomes
 *      a new property with status 'prospect' — the warehouse's own bucket for a property that is not for sale —
 *      because a Regrid parcel is land with a registry identity, not a listing.
 *   3. A PERSON IS MATCHED BY SOURCE KEY, NEVER BY GUESS. `integration_source_person_link` carries source
 *      'regrid' and a key of owner name plus mailing ZIP, which is what separates two owners who share a name.
 *      When the key is new, EXACTLY ONE person whose display_name matches is adopted and enriched (fill-only);
 *      two or more are AMBIGUOUS and reported instead of guessed; none means a new person.
 *   4. CONFLICTS ARE REPORTED, NOT RESOLVED SILENTLY. Every parent column where the warehouse already holds a
 *      different value is counted and shown with examples, so "the better source won" is auditable.
 *   5. ONE TRANSACTION, AND A DRY RUN IS THE SAME WORK ROLLED BACK. A failure leaves the warehouse untouched and
 *      `--dry-run` prints exactly the plan the real run would execute.
 *
 * THE LOAD IS BATCHED, NOT PER-ROW (measured). The first version issued 6–8 queries per parcel; against Neon that
 * is thousands of round trips and a dry run that had not finished after five minutes. Decisions are therefore made
 * IN MEMORY against prefetched indexes — this is a landing pipeline, not an interactive one — and written in
 * chunks with one multi-row statement each. A single INSERT .. RETURNING returns rows in input order, so ids zip
 * back onto the tuples that produced them. This matters because the operator buys this data again.
 */
/**
 * The values this row offers the PARENT columns. Only what the export actually has; a blank source value never
 * touches a parent column, which is what keeps a human entry safe.
 */
function promotions(row: Row): Record<string, string | null> {
  const catastro = compact(row.catastro) ?? compact(row.parcelnumb)
  return {
    address_line1: compact(row.address) ?? compact(row.direccion_fisica),
    city: compact(row.scity) ?? compact(row.city),
    state_or_province: compact(row.state2),
    postal_code: compact(row.szip5) ?? compact(row.szip),
    legal_owner_name: compact(row.owner),
    latitude: num(row.lat),
    longitude: num(row.lon),
    // `ll_gisacre` is the GIS acreage. `cabida` is the Puerto Rico registered area in SQUARE METRES and is kept in
    // its own source column rather than converted into a unit nobody wrote down.
    lot_size: num(row.ll_gisacre),
    year_built: num(row.yearbuilt),
    stories: num(row.numstories),
    bedrooms: num(row.num_bedrooms),
    bathrooms: num(row.num_bath),
    square_feet: num(row.ll_gissqft),
    listing_identifier: catastro,
  }
}

/** The source's own record of this parcel: always written, never merged with anything human. */
function sourceColumns(row: Row): Record<string, string | boolean | null> {
  return {
    regrid_ll_uuid: compact(row.ll_uuid),
    regrid_parcel_number: compact(row.parcelnumb) ?? compact(row.catastro),
    regrid_path: compact(row.path),
    regrid_match_address: compact(row.address) ?? compact(row.direccion_fisica),
    regrid_geoid: compact(row.geoid),
    regrid_usecode: compact(row.usecode),
    regrid_usedesc: compact(row.usedesc),
    regrid_zoning: compact(row.zoning) ?? compact(row.zoning_description),
    regrid_land_value: num(row.landval),
    regrid_improvement_value: num(row.improvval),
    regrid_total_value: num(row.parval),
    regrid_taxable_value: num(row.taxable),
    regrid_sale_price: num(row.saleprice),
    regrid_sale_date: dateOnly(row.saledate),
    regrid_cabida: num(row.cabida),
    regrid_owner_name: compact(row.owner),
    regrid_owner_mailing: compact(row.mailadd),
    regrid_qoz: /^(yes|y|true|t|1)$/i.test(compact(row.qoz) ?? '') ? true : null,
    // Promoted by migration 202 from the fields that carry data. `estate` is NOT here on purpose: sampling showed
    // its values are record references ('1522', '763', 'N/A'), not a yes/no, so it stays in the receipt table.
    // `ll_gisacre`, NOT `gisacre`. Measured: `gisacre` is non-null in all 2138 rows but holds an EMPTY STRING, so
    // the first version of this mapping promoted nothing and the column filled 0 rows while its siblings filled.
    // `ll_gisacre` is the field that actually carries the acreage — the same one `lot_size` already reads.
    regrid_gis_acreage: num(row.ll_gisacre),
    regrid_gis_square_feet: num(row.ll_gissqft),
    regrid_census_tract: compact(row.census_tract),
    regrid_land_type: compact(row.tipo),
    regrid_address_source: compact(row.address_source),
    regrid_num_catastro: compact(row.num_catastro),
    regrid_machinery: num(row.machinery),
    regrid_exemption: num(row.exemp),
    regrid_exoneration: num(row.exon),
    regrid_municipio: compact(row.municipio),
    regrid_old_parcel_id: compact(row.oldpid),
    regrid_stable_id: compact(row.ll_stable_id),
    regrid_original_address: compact(row.original_address),
    regrid_deed_number: compact(row.deednum),
    regrid_buyer_name: compact(row.buyername),
    regrid_previous_owner: compact(row.previous_owner),
    regrid_registry_book: compact(row.book),
    regrid_registry_page: compact(row.page),
    regrid_urbanization: compact(row.urbanization),
    regrid_source_url: compact(row.url) ?? compact(row.sourceurl),
    regrid_lookup_query: `table:${TABLE}`,
    regrid_data: JSON.stringify(row),
  }
}

/**
 * One line per conflicting parent column, so "the better source won" is auditable rather than asserted.
 *
 * A DIFFERENCE IS NOT ALWAYS A CONFLICT, and the first version of this report cried wolf: `property.latitude` is
 * stored at 7 decimal places and `property.lot_size` at 2, so a value this loader itself wrote came back looking
 * different ("18.3093850" vs "18.309385", "32.47" vs "32.46603") and every re-run reported dozens of conflicts that
 * were nothing but the column's storage precision. Numbers are therefore compared with a tolerance proportional to
 * the storage — 0.01 absolute or 1e-4 relative — which treats those pairs as the SAME fact while still reporting a
 * genuine disagreement. Text is compared case- and space-insensitively, so Regrid's "BO DELICIAS 17A" against a
 * hand-entered "Bo. Delicias 17a" is still reported, because that is a real difference a human made.
 */
function sameValue(held: string, offered: string): boolean {
  const left = Number(held)
  const right = Number(offered)
  if (Number.isFinite(left) && Number.isFinite(right)) {
    return Math.abs(left - right) <= Math.max(0.01, Math.abs(right) * 1e-4)
  }
  const normalize = (value: string): string => value.replace(/\s+/g, ' ').trim().toUpperCase()
  return normalize(held) === normalize(offered)
}

function conflicts(row: Row, existing: Record<string, unknown>): string[] {
  const found: string[] = []
  const proposed = promotions(row)
  for (const column of Object.keys(proposed)) {
    const held = compact(existing[column])
    const offered = proposed[column]
    if (held !== null && offered !== null && !sameValue(held, offered)) {
      found.push(`${column}: kept ${JSON.stringify(held)}, Regrid offered ${JSON.stringify(offered)}`)
    }
  }
  return found
}

class DryRun extends Error {}

/** The columns a NEW property is created with. The source half is DERIVED from `sourceColumns`, so the column
 * list and the value list cannot drift apart — an earlier version listed them independently and Postgres caught
 * the mismatch as "bind message supplies 9400 parameters, but prepared statement requires 5200". */
const PARENT_INSERT_COLUMNS = [
  'name',
  'status',
  'address_line1',
  'city',
  'state_or_province',
  'postal_code',
  'country',
  'iso_country_code',
  'legal_owner_name',
  'latitude',
  'longitude',
  'lot_size',
  'lot_size_units',
  'year_built',
  'stories',
  'bedrooms',
  'bathrooms',
  'square_feet',
  'listing_identifier',
  'catastro_source',
  'source_type',
  'source_provider',
  'source_listing_key',
  'last_synced_at',
  'regrid_enriched_at',
  'regrid_loaded_at',
] as const

// The source's own columns, taken from `sourceColumns` itself: its keys do not depend on the row, so the column
// list and the value list agree BY CONSTRUCTION rather than by two lists being kept in step. Read lazily, because
// `sourceColumns` uses helpers declared later in this file and module-level evaluation would hit them too early.
function propertyInsertColumns(): string[] {
  return [...PARENT_INSERT_COLUMNS, ...Object.keys(sourceColumns({}))]
}

/**
 * A parcel is LAND, not a listing: status 'prospect' (this warehouse's bucket for a property that is not for
 * sale), no property_type (Regrid's use fields are empty in this export and inventing a type would be a lie),
 * and source_type/source_provider set so nothing downstream mistakes a parcel for a listing.
 */
function propertyInsertValues(
  row: Row,
  nowIso: string,
): Array<string | boolean | null> {
  const parent = promotions(row)
  const catastro = parent.listing_identifier
  return [
    null,
    'prospect',
    parent.address_line1,
    parent.city,
    parent.state_or_province,
    parent.postal_code,
    'Puerto Rico',
    'us',
    parent.legal_owner_name,
    parent.latitude,
    parent.longitude,
    parent.lot_size,
    parent.lot_size === null ? null : 'acres',
    parent.year_built,
    parent.stories,
    parent.bedrooms,
    parent.bathrooms,
    parent.square_feet,
    catastro,
    catastro === null ? null : SOURCE,
    SOURCE,
    'Regrid',
    compact(row.ll_uuid),
    nowIso,
    nowIso,
    nowIso,
    ...Object.values(sourceColumns(row)),
  ]
}

type Counters = {
  feedRows: number
  propertiesCreated: number
  propertiesEnriched: number
  ownersLinked: number
  ownersCreated: number
  ownersAdopted: number
  ownersAmbiguous: number
  relations: number
  legalAddresses: number
}

function emptyCounters(): Counters {
  return {
    feedRows: 0,
    propertiesCreated: 0,
    propertiesEnriched: 0,
    ownersLinked: 0,
    ownersCreated: 0,
    ownersAdopted: 0,
    ownersAmbiguous: 0,
    relations: 0,
    legalAddresses: 0,
  }
}

/** One person-role on one parcel: the owner, or the previous owner, or the buyer named in the record. */
type Fact = { key: string; name: string; propertyId: string; relationType: string; legalAddress: LegalAddress | null }

type LegalAddress = {
  line1: string | null
  line2: string | null
  city: string | null
  state: string | null
  postalCode: string | null
  country: string | null
}

function chunk<T>(items: readonly T[]): T[][] {
  const out: T[][] = []
  for (let start = 0; start < items.length; start += CHUNK) out.push(items.slice(start, start + CHUNK))
  return out
}

function insertStatement(table: string, columns: readonly string[], rows: number): string {
  const tuples: string[] = []
  let at = 0
  for (let row = 0; row < rows; row++) {
    tuples.push(`(${columns.map(() => `$${++at}`).join(', ')})`)
  }
  return `insert into ${table} (${columns.map((column) => `"${column}"`).join(', ')}) values ${tuples.join(', ')}`
}

import { createHash } from 'node:crypto'
import { withForgeTransaction, type ForgeDbTarget, type ForgeDbTx } from '../db/forge-db'

const args = process.argv.slice(2)
const argOf = (name: string): string | null => {
  const at = args.indexOf(`--${name}`)
  return at >= 0 ? (args[at + 1] ?? null) : null
}
const target: ForgeDbTarget = argOf('target') === 'prod' ? 'prod' : argOf('target') === 'dev' ? 'dev' : process.env.APP_ENV === 'production' ? 'prod' : 'dev'
const dryRun = args.includes('--dry-run')
const limit = argOf('limit') ? Number(argOf('limit')) : null

const SOURCE = 'regrid'
/** The dataset this identity came from, recorded the way the other external sources record their account. */
const SOURCE_ACCOUNT = 'regrid:parcels'
const TABLE = 'l_Regrid'
const CHUNK = 200

const out = (line: string): void => {
  process.stdout.write(`${line}\n`)
}

type Row = Record<string, unknown>

const compact = (value: unknown): string | null => {
  if (value === null || value === undefined) return null
  const text = String(value).trim()
  return text === '' ? null : text
}

const num = (value: unknown): string | null => {
  const text = compact(value)
  if (text === null) return null
  return Number.isFinite(Number(text)) ? text : null
}

/**
 * A `date` column comes back from node-postgres as a JS Date, and handing that Date straight back to Postgres
 * serialises it with a timezone NAME ("gmt-0400") that the server rejects — measured on the first dry run.
 */
const dateOnly = (value: unknown): string | null => {
  if (value instanceof Date) return value.toISOString().slice(0, 10)
  const text = compact(value)
  return text === null ? null : text.slice(0, 10)
}

/** The owner key: name plus mailing ZIP. The ZIP is what separates two owners who share a name. */
export function ownerKey(owner: unknown, mailZip: unknown): string | null {
  const name = compact(owner)
  if (name === null) return null
  const normalized = name
    .toUpperCase()
    .replace(/[.,]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim()
  if (normalized === '') return null
  const zip = (compact(mailZip) ?? '').replace(/\D/g, '').slice(0, 5)
  return `${normalized}|${zip}`
}

/** An owner name that reads like an entity gets `company` filled; a natural person does not. */
export function looksLikeEntity(owner: string): boolean {
  return /\b(LLC|L\.L\.C|CORP|INC|INCORPORATED|COMPANY|CO\b|LTD|LP|LLP|TRUST|ESTATE|HOLDINGS|GROUP|BANK|PARTNERS|ASSOCIATION|CHURCH|AUTHORITY)\b/i.test(
    owner,
  )
}

type Existing = { id: string; [column: string]: unknown }

/** Three indexes, read once: two ways to recognise a property, and the identities already linked to a person. */
async function prefetch(tx: ForgeDbTx): Promise<{
  byUuid: Map<string, Existing>
  byCatastro: Map<string, Existing>
  linkedPersons: Map<string, string>
  personsByName: Map<string, string[]>
}> {
  const properties = (
    await tx.query(
      `select id, regrid_ll_uuid, listing_identifier, address_line1, city, state_or_province, postal_code,
              legal_owner_name, latitude, longitude, lot_size, year_built, stories, bedrooms, bathrooms, square_feet
         from property`,
    )
  ).rows as Existing[]
  const byUuid = new Map<string, Existing>()
  const byCatastro = new Map<string, Existing>()
  for (const property of properties) {
    const uuid = compact(property.regrid_ll_uuid)
    if (uuid) byUuid.set(uuid, property)
    const catastro = compact(property.listing_identifier)
    if (catastro) byCatastro.set(catastro, property)
  }

  const links = (
    await tx.query(
      `select source_identity_key, canonical_person_id from integration_source_person_link where source = $1`,
      [SOURCE],
    )
  ).rows as Array<{ source_identity_key: string; canonical_person_id: string }>
  const linkedPersons = new Map(links.map((link) => [link.source_identity_key, link.canonical_person_id]))

  const persons = (
    await tx.query(`select id, lower(btrim(display_name)) as name_key from person where archived_at is null`)
  ).rows as Array<{ id: string; name_key: string }>
  const personsByName = new Map<string, string[]>()
  for (const person of persons) {
    const key = String(person.name_key)
    personsByName.set(key, [...(personsByName.get(key) ?? []), person.id])
  }

  return { byUuid, byCatastro, linkedPersons, personsByName }
}

/**
 * THE RECEIPT: one row per parcel in `regrid_culebra_parcel`, the 1:1 mirror of the landing table.
 *
 * `property` holds what something reads; this holds EVERYTHING WE BOUGHT, in the source's own values, so a field
 * nobody promoted yet is still queryable as a typed column and `raw_payload` keeps the literal record. The column
 * list is read FROM THE TABLE rather than written twice, so a field added to the feed table is picked up the next
 * time this runs instead of quietly not being written — the same by-construction rule the property insert uses.
 * Upserted on `ll_uuid`, which is its primary key, so a second run rewrites rather than duplicates.
 */
async function mirrorFeed(tx: ForgeDbTx, rows: Row[], nowIso: string): Promise<number> {
  const catalogue = (
    await tx.query(
      `select column_name from information_schema.columns where table_name = 'regrid_culebra_parcel' order by ordinal_position`,
    )
  ).rows as Array<{ column_name: string }>
  const tableColumns = catalogue.map((entry) => String(entry.column_name))
  const special = new Set([
    'raw_payload',
    'loaded_at',
    'source_row_sha256',
    'source_row_number',
    'source_file_sha256',
  ])
  const fields = new Set(Object.keys(rows[0] ?? {}))

  const columns = [
    'll_uuid',
    ...tableColumns.filter((column) => column !== 'll_uuid' && (special.has(column) || fields.has(column))),
  ]
  // A TABLE FEED HAS NO FILE TO HASH, so `source_file_sha256` carries the content digest of the batch instead: the
  // hash of every row hash in order. That is the receipt's fingerprint — it answers "did this purchase change
  // anything" without comparing 2138 rows by eye, and it is what the column was created for.
  const rowHashes = rows.map((row) => createHash('sha256').update(JSON.stringify(row)).digest('hex'))
  const feedDigest = createHash('sha256').update(rowHashes.join('')).digest('hex')

  const valueFor = (row: Row, column: string, rowNumber: number): string | number | null => {
    if (column === 'raw_payload') return JSON.stringify(row)
    if (column === 'loaded_at') return nowIso
    if (column === 'source_row_number') return rowNumber
    if (column === 'source_row_sha256') return rowHashes[rowNumber] ?? null
    if (column === 'source_file_sha256') return feedDigest
    const value = row[column]
    if (value === null || value === undefined) return null
    if (value instanceof Date) return value.toISOString()
    return typeof value === 'number' ? value : String(value)
  }

  for (let start = 0; start < rows.length; start += CHUNK) {
    const part = rows.slice(start, start + CHUNK)
    const params = part.flatMap((row, at) => columns.map((column) => valueFor(row, column, start + at)))
    const updates = columns.filter((column) => column !== 'll_uuid').map((column) => `"${column}" = excluded."${column}"`)
    await tx.query(
      `${insertStatement('regrid_culebra_parcel', columns, part.length)}
         on conflict (ll_uuid) do update set ${updates.join(', ')}`,
      params,
    )
  }
  return rows.length
}
async function load(
  tx: ForgeDbTx,
  counters: Counters,
  conflictsReport: string[],
  personReport: string[],
): Promise<number> {
  const rows = (
    await tx.query(`select * from "${TABLE}" order by "ogc_fid"${limit ? ` limit ${limit}` : ''}`)
  ).rows as Row[]
  out(`source: "${TABLE}" — ${rows.length} parcels  ->  target: ${target}${dryRun ? '   (DRY RUN)' : ''}`)

  const index = await prefetch(tx)
  const nowIso = new Date().toISOString()

  // THE RECEIPT FIRST: the whole purchase lands in the 1:1 feed table before anything is promoted from it, so the
  // end points can be rebuilt from the receipt alone without another purchase.
  counters.feedRows = await mirrorFeed(tx, rows, nowIso)
  out(`feed table: ${counters.feedRows} parcels mirrored 1:1 into regrid_culebra_parcel`)

  // ---- PROPERTIES: decide in memory, then write in chunks.
  const toInsert: Row[] = []
  const toUpdate: Array<{ row: Row; existing: Existing }> = []
  for (const row of rows) {
    const existing = match(row, index)
    if (existing) toUpdate.push({ row, existing })
    else toInsert.push(row)
  }
  if (toUpdate.length > 0) {
    out(`existing properties matched by identity: ${toUpdate.length} (enriched fill-only; conflicts listed below)`)
  }

  const propertyIdByRow = new Map<Row, string>()
  const insertIds: string[] = []
  for (const part of chunk(toInsert)) {
    const params = part.flatMap((row) => propertyInsertValues(row, nowIso))
    const written = await tx.query(
      `${insertStatement('property', propertyInsertColumns(), part.length)} returning id`,
      params,
    )
    for (const writtenRow of written.rows) insertIds.push(String(writtenRow.id))
  }
  toInsert.forEach((row, at) => propertyIdByRow.set(row, insertIds[at] as string))
  counters.propertiesCreated = insertIds.length

  for (const { row, existing } of toUpdate) {
    const propertyId = String(existing.id)
    propertyIdByRow.set(row, propertyId)
    const found = conflicts(row, existing)
    if (found.length > 0 && conflictsReport.length < 15) {
      conflictsReport.push(
        `property ${propertyId} (${compact(row.parcelnumb) ?? 'no parcel number'}) — ${found.join(' | ')}`,
      )
    }
    const parent = promotions(row)
    const sets: string[] = []
    const params: Array<string | boolean | null> = [propertyId]
    // THE PARENT'S OWN COLUMNS ARE FILL-ONLY. A blank is filled; anything a human or an import already put there
    // is left exactly as it is — which is what makes the hand-entered form records the better source.
    for (const [column, value] of Object.entries(parent)) {
      if (value === null) continue
      params.push(value)
      sets.push(`"${column}" = coalesce("${column}", $${params.length})`)
    }
    if (compact(existing.listing_identifier) === null && parent.listing_identifier !== null) {
      params.push(SOURCE)
      sets.push(`"catastro_source" = $${params.length}`)
    }
    // THE SOURCE'S OWN RECORD IS ALWAYS THE SOURCE'S. `regrid_ll_uuid` is the identity, never reassigned.
    for (const [column, value] of Object.entries(sourceColumns(row))) {
      params.push(value)
      sets.push(
        column === 'regrid_ll_uuid'
          ? `"regrid_ll_uuid" = coalesce("regrid_ll_uuid", $${params.length})`
          : `"${column}" = $${params.length}`,
      )
    }
    sets.push('"regrid_enriched_at" = now()', '"regrid_loaded_at" = now()', '"last_synced_at" = now()', '"updated_at" = now()')
    await tx.query(`update property set ${sets.join(', ')} where id = $1`, params)
    counters.propertiesEnriched += 1
  }

  return afterProperties(tx, rows, propertyIdByRow, index, counters, personReport)
}

function match(row: Row, index: Awaited<ReturnType<typeof prefetch>>): Existing | null {
  const uuid = compact(row.ll_uuid)
  if (uuid && index.byUuid.has(uuid)) return index.byUuid.get(uuid) as Existing
  const catastro = compact(row.catastro) ?? compact(row.parcelnumb)
  if (catastro && index.byCatastro.has(catastro)) return index.byCatastro.get(catastro) as Existing
  return null
}

/**
 * OWNER PERSONS. Three roles come out of the record and each is a person, not a column: the current `owner`, the
 * `previous_owner` (1122 parcels) and the `buyername` (1171). A source key already linked is reused; exactly one
 * namesake is adopted and enriched; two or more are reported instead of guessed; none means a new person.
 *
 * THE OWNER'S ADDRESS OF RECORD — what the operator calls the legal address, frequently a PO Box — is written to
 * the PERSON, never to the parcel, and the tie is recorded as `legal_address` because that is the word this
 * warehouse already uses (`person_property_relation_type_check`). The parcel keeps its own address separately.
 */
function legalAddressOf(row: Row): LegalAddress | null {
  const line1 =
    compact(row.original_mailing_address) ?? compact(row.mailadd) ?? compact(row.direccion_postal) ?? null
  const address: LegalAddress = {
    line1,
    line2: compact(row.mail_address2),
    city: compact(row.mail_city),
    state: compact(row.mail_state2),
    postalCode: compact(row.mail_zip),
    country: compact(row.mail_country),
  }
  const any = Object.values(address).some((value) => value !== null)
  return any ? address : null
}

function factsFor(row: Row, propertyId: string): Fact[] {
  const facts: Fact[] = []
  const owner = compact(row.owner)
  const ownerKeyValue = owner === null ? null : ownerKey(owner, row.mail_zip)
  if (owner !== null && ownerKeyValue !== null) {
    facts.push({
      key: ownerKeyValue,
      name: owner,
      propertyId,
      relationType: 'physical_property',
      legalAddress: legalAddressOf(row),
    })
  }
  // A prior owner or a buyer has no address in the record that belongs to them, so their key is the name alone —
  // which is exactly why the namesake rule exists rather than a fuzzy match.
  for (const [field, relationType] of [
    ['previous_owner', 'previous_owner'],
    ['buyername', 'buyer'],
  ] as const) {
    const name = compact(row[field])
    if (name === null) continue
    const key = ownerKey(name, '')
    if (key === null) continue
    facts.push({ key, name, propertyId, relationType, legalAddress: null })
  }
  return facts
}

async function afterProperties(
  tx: ForgeDbTx,
  rows: Row[],
  propertyIdByRow: Map<Row, string>,
  index: Awaited<ReturnType<typeof prefetch>>,
  counters: Counters,
  personReport: string[],
): Promise<number> {
  const facts: Fact[] = []
  for (const row of rows) {
    const propertyId = propertyIdByRow.get(row)
    if (!propertyId) continue
    facts.push(...factsFor(row, propertyId))
  }

  const identities = new Map<string, { name: string; location: string | null; legalAddress: LegalAddress | null }>()
  for (const fact of facts) {
    const entry = identities.get(fact.key) ?? { name: fact.name, location: null, legalAddress: null }
    if (entry.legalAddress === null && fact.legalAddress !== null) entry.legalAddress = fact.legalAddress
    entry.location =
      entry.location ??
      (fact.legalAddress === null
        ? null
        : [fact.legalAddress.city, fact.legalAddress.state].filter(Boolean).join(', ') || null)
    identities.set(fact.key, entry)
  }

  const resolved = new Map<string, string>()
  const toCreate: Array<{ key: string; person: { name: string; location: string | null; legalAddress: LegalAddress | null } }> = []
  for (const [key, person] of identities) {
    const linked = index.linkedPersons.get(key)
    if (linked) {
      resolved.set(key, linked)
      continue
    }
    const namesakes = index.personsByName.get(person.name.toLowerCase()) ?? []
    if (namesakes.length > 1) {
      counters.ownersAmbiguous += 1
      if (personReport.length < 12) {
        personReport.push(`${JSON.stringify(person.name)} has ${namesakes.length} namesakes — left alone`)
      }
      continue
    }
    if (namesakes.length === 1) {
      resolved.set(key, namesakes[0] as string)
      counters.ownersAdopted += 1
      continue
    }
    toCreate.push({ key, person })
  }

  const createdIds: string[] = []
  for (const part of chunk(toCreate)) {
    const params = part.flatMap(({ person }) => [
      person.name,
      SOURCE,
      'new',
      'unclassified',
      looksLikeEntity(person.name) ? person.name : null,
      person.location,
      person.legalAddress?.line1 ?? null,
      person.legalAddress?.line2 ?? null,
      person.legalAddress?.city ?? null,
      person.legalAddress?.state ?? null,
      person.legalAddress?.postalCode ?? null,
      person.legalAddress?.country ?? null,
      person.legalAddress === null ? null : SOURCE,
    ])
    const written = await tx.query(
      `${insertStatement(
        'person',
        [
          'display_name',
          'display_name_source',
          'status',
          'role',
          'company',
          'location',
          'legal_address_line1',
          'legal_address_line2',
          'legal_address_city',
          'legal_address_state',
          'legal_address_postal_code',
          'legal_address_country',
          'legal_address_source',
        ],
        part.length,
      )} returning id`,
      params,
    )
    for (const writtenRow of written.rows) createdIds.push(String(writtenRow.id))
  }
  const createdKeys = new Set(toCreate.map((entry) => entry.key))
  toCreate.forEach((entry, at) => resolved.set(entry.key, createdIds[at] as string))
  counters.ownersCreated = createdIds.length
  counters.ownersLinked = resolved.size

  // FILL-ONLY, like every other parent column: an existing person keeps whatever anyone already knew. The legal
  // address fills the same way — an address somebody already recorded is not replaced by one a vendor supplied.
  //
  // BATCHED, because the first version issued one UPDATE per person and that is thousands of round trips: the same
  // mistake the property side already had fixed, measured at over three minutes against Neon. Each chunk is ONE
  // statement joining a VALUES list on the id.
  const enrichment: Array<Array<string | null>> = []
  for (const [key, personId] of resolved) {
    const person = identities.get(key)
    if (person?.legalAddress) counters.legalAddresses += 1
    enrichment.push([
      personId,
      person !== undefined && looksLikeEntity(person.name) ? person.name : null,
      person?.location ?? null,
      person?.legalAddress?.line1 ?? null,
      person?.legalAddress?.line2 ?? null,
      person?.legalAddress?.city ?? null,
      person?.legalAddress?.state ?? null,
      person?.legalAddress?.postalCode ?? null,
      person?.legalAddress?.country ?? null,
      SOURCE,
    ])
  }
  for (const part of chunk(enrichment)) {
    const tuples = part
      .map((_, at) => {
        const base = at * 10
        return `($${base + 1}::uuid, $${base + 2}, $${base + 3}, $${base + 4}, $${base + 5}, $${base + 6}, $${base + 7}, $${base + 8}, $${base + 9}, $${base + 10}::text)`
      })
      .join(', ')
    await tx.query(
      `update person p
          set company = coalesce(p.company, v.company),
              location = coalesce(p.location, v.location),
              legal_address_line1 = coalesce(p.legal_address_line1, v.line1),
              legal_address_line2 = coalesce(p.legal_address_line2, v.line2),
              legal_address_city = coalesce(p.legal_address_city, v.city),
              legal_address_state = coalesce(p.legal_address_state, v.state),
              legal_address_postal_code = coalesce(p.legal_address_postal_code, v.postal_code),
              legal_address_country = coalesce(p.legal_address_country, v.country),
              legal_address_source = case
                when p.legal_address_source is null and v.line1 is not null then v.source
                else p.legal_address_source
              end,
              updated_at = now()
         from (values ${tuples}) as v(id, company, location, line1, line2, city, state, postal_code, country, source)
        where p.id = v.id`,
      part.flat(),
    )
  }

  // LINKS: the prefetch read every existing 'regrid' link, so a key that was not in it is certainly absent.
  const linkRows: string[][] = []
  for (const [key, personId] of resolved) {
    if (index.linkedPersons.has(key)) continue
    linkRows.push([
      SOURCE,
      SOURCE_ACCOUNT,
      key,
      personId,
      createdKeys.has(key) ? 'new_identity' : 'exact_identity',
      `Regrid parcel person (${TABLE})`,
    ])
  }
  for (const part of chunk(linkRows)) {
    await tx.query(
      insertStatement(
        'integration_source_person_link',
        ['source', 'source_account', 'source_identity_key', 'canonical_person_id', 'link_method', 'link_reason'],
        part.length,
      ),
      part.flat(),
    )
  }

  // TIES: every role in the record becomes a tie, and the existing ties are read WITH their relation_type so a
  // second run adds nothing twice. The vocabulary is the warehouse's own, widened by migration 203 for
  // `previous_owner` and `buyer` rather than bending those people into `interest`, which would be a lie.
  const existingTies = (
    await tx.query('select person_id, property_id, relation_type from person_property where source_type = $1', [SOURCE])
  ).rows as Array<{ person_id: string; property_id: string; relation_type: string }>
  const tieSeen = new Set(existingTies.map((tie) => `${tie.person_id}|${tie.property_id}|${tie.relation_type}`))
  const tieRows: string[][] = []
  for (const fact of facts) {
    const personId = resolved.get(fact.key)
    if (!personId) continue
    const tie = `${personId}|${fact.propertyId}|${fact.relationType}`
    if (tieSeen.has(tie)) continue
    tieSeen.add(tie)
    tieRows.push([personId, fact.propertyId, fact.relationType, 'current', SOURCE, fact.key])
    // The owner's ADDRESS OF RECORD is a relation of its own in this warehouse's vocabulary (`legal_address`), so
    // the legal address is reachable from the person AND from the parcel without mistaking it for the parcel's own
    // address. Only the owner row carries an address in this record; a prior owner or a buyer does not.
    if (fact.legalAddress !== null) {
      const legalTie = `${personId}|${fact.propertyId}|legal_address`
      if (!tieSeen.has(legalTie)) {
        tieSeen.add(legalTie)
        tieRows.push([personId, fact.propertyId, 'legal_address', 'current', SOURCE, fact.key])
      }
    }
  }
  counters.relations = tieRows.length
  for (const part of chunk(tieRows)) {
    await tx.query(
      insertStatement(
        'person_property',
        ['person_id', 'property_id', 'relation_type', 'relation_status', 'source_type', 'source_key'],
        part.length,
      ),
      part.flat(),
    )
  }
  counters.relations = tieRows.length

  // THE LOAD PROVES ITS OWN SHAPE before it is allowed to commit: every parcel this run touched must be reachable
  // in the warehouse by its Regrid identity, and the owner ties must exist beside it. The dry run runs all of this
  // and then throws, so the transaction rolls back and the warehouse is untouched.
  const loaded = await tx.query('select count(*)::int as n from property where regrid_ll_uuid is not null')
  const tied = await tx.query("select count(*)::int as n from person_property where source_type = 'regrid'")
  out(
    `warehouse now holds ${String(loaded.rows[0]?.n)} Regrid-identified properties, ` +
      `${String(tied.rows[0]?.n)} Regrid owner ties`,
  )
  if (dryRun) throw new DryRun()
  return rows.length
}

async function main(): Promise<void> {
  const counters = emptyCounters()
  const conflictsReport: string[] = []
  const personReport: string[] = []

  const summary = (): void => {
    out('')
    out(`feed table rows (1:1 receipt): ${counters.feedRows}`)
    out(`properties created: ${counters.propertiesCreated}`)
    out(`properties enriched (existing rows, fill-only): ${counters.propertiesEnriched}`)
    out(
      `persons — created: ${counters.ownersCreated}  adopted by exact name: ${counters.ownersAdopted}  ` +
        `ambiguous, left alone: ${counters.ownersAmbiguous}  distinct identities: ${counters.ownersLinked}`,
    )
    out(`ties written (owner / previous owner / buyer / legal address): ${counters.relations}`)
    out(`persons carrying an address of record from this feed: ${counters.legalAddresses}`)
    if (conflictsReport.length > 0) {
      out('')
      out(`conflicts — the warehouse value was KEPT (${conflictsReport.length} shown):`)
      for (const line of conflictsReport) out(`  ${line}`)
    }
    if (personReport.length > 0) {
      out('')
      out('owners left unlinked:')
      for (const line of personReport) out(`  ${line}`)
    }
  }

  try {
    const parcels = await withForgeTransaction(
      (tx) => load(tx, counters, conflictsReport, personReport),
      target,
    )
    summary()
    out(`committed: ${parcels} parcels processed into ${target}`)
  } catch (error) {
    if (error instanceof DryRun) {
      summary()
      out('')
      out('dry run: rolled back — the warehouse is exactly as it was')
      return
    }
    throw error
  }
}

main().catch((error: unknown) => {
  process.stderr.write(`load-regrid-warehouse: ${error instanceof Error ? error.message : String(error)}\n`)
  process.exit(1)
})

