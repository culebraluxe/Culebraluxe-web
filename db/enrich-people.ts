// ---------------------------------------------------------------------------
// REL-INTEL — canonical Person enrichment from Apple Contacts staged data.
//
// APPLE CONTACTS L DATA  +  APPLE MESSAGES IDENTITIES
//        ↓
// normalize phone/email  →  match identity  →  canonical person
//
// Rules:
//   - Never replace a real human name with a phone number / email.
//   - If an Apple Contacts staged record matches a canonical Person identity,
//     enrich person.display_name from the Contact human name and carry
//     supported contact profile fields (location) — phone/email stay in
//     person_identity.
//   - Never create a duplicate Person because Contacts and Messages arrived
//     separately; this module only updates existing canonical Persons.
//   - If no human name resolves from any trusted source, the phone/email
//     fallback may remain, but the person is marked 'unresolved' rather than
//     treating the identity string as a good client name.
//   - Preserve source provenance (person.display_name_source) and stay
//     idempotent / replay-safe (only identity-fallback / unmarked persons are
//     considered; enriched names are human and stop being candidates).
// ---------------------------------------------------------------------------
import { sql } from './client'
import { normalizePhone, normalizeEmail } from '../lib/relationship-intel/normalize'
import { isHumanName } from '../lib/relationship-intel/names'
import { refreshClientReadModels } from './client-read-models'

export type ContactInfo = {
  displayName: string | null
  organization: string | null
  displayAddress: string | null
  /** When this contact revision was last projected — the recency signal for "newer wins". */
  updatedAt?: string | null
}

export type EnrichResult = {
  enriched: number
  unresolved: number
  ambiguous: number
  resolvedHuman: number
}

/** Normalize an identity (type + value) into a stable match key. */
export function identityMatchKey(
  type: string,
  value: string | null | undefined,
): string | null {
  if (!value) return null
  if (type === 'email') {
    const n = normalizeEmail(value)
    return n.ok ? `email:${n.value}` : null
  }
  if (type === 'phone') {
    const n = normalizePhone(value)
    return n.ok ? `phone:${n.value}` : null
  }
  return `${type}:${value}`
}

/**
 * Build an identity -> Apple Contacts staged records index. One identity key
 * may map to several Contacts (same number stored under different names).
 */
export function buildContactIndex(
  contacts: Array<
    ContactInfo & { identityType: string; normalizedValue: string | null }
  >,
): Map<string, ContactInfo[]> {
  const index = new Map<string, ContactInfo[]>()
  for (const c of contacts) {
    const key = identityMatchKey(c.identityType, c.normalizedValue)
    if (!key) continue
    const info: ContactInfo = {
      displayName: c.displayName,
      organization: c.organization,
      displayAddress: c.displayAddress,
      updatedAt: c.updatedAt ?? null,
    }
    const arr = index.get(key) ?? []
    arr.push(info)
    index.set(key, arr)
  }
  return index
}

export type IdentityResolution = {
  contact: ContactInfo | null
  ambiguous: boolean
}

/**
 * Resolve a canonical Person's identity keys against Apple Contacts.
 *
 * NEWER WINS (captain's rule, 2026-09-10 — the same problem as reused tickers in
 * corporate actions). Apple does NOT enforce unique numbers, so one identity commonly
 * matches several contacts with different names. The previous behaviour refused to
 * decide ("ambiguous -> keep what we have"), which let an OLD, poorer record beat a
 * newer, fully-qualified one: "Puerple House" (a colour used as a memory aid in the
 * name field) outlived the real "Juan A. Santa Cruz" on the same phone number.
 *
 * Recency is the tie-breaker, then completeness; old data is simply deprecated. A
 * single distinct human name still resolves directly, and zero human names still
 * resolves to nothing (never invent).
 */
export function resolveContactForIdentityKeys(
  keys: string[],
  index: Map<string, ContactInfo[]>,
): IdentityResolution {
  const matches: ContactInfo[] = []
  for (const key of keys) {
    const arr = index.get(key)
    if (arr) matches.push(...arr)
  }
  const human = matches.filter((c) => isHumanName(c.displayName))
  if (!human.length) return { contact: null, ambiguous: false }

  const distinctNames = new Set(human.map((c) => c.displayName!.trim()))
  if (distinctNames.size === 1) {
    const name = [...distinctNames][0]
    const contact = human.find((c) => c.displayName!.trim() === name) ?? null
    return { contact, ambiguous: false }
  }

  // Conflicting names across duplicate/shared contacts: take the NEWEST, then the most
  // complete. Never fall back to the caller's older stored value.
  const newest = [...human].sort(
    (a, b) =>
      recencyOf(b) - recencyOf(a) ||
      completenessOf(b) - completenessOf(a) ||
      (a.displayName ?? '').localeCompare(b.displayName ?? ''),
  )[0]
  return { contact: newest ?? null, ambiguous: false }
}

function recencyOf(c: ContactInfo): number {
  const t = c.updatedAt ? Date.parse(c.updatedAt) : NaN
  return Number.isFinite(t) ? t : 0
}

function completenessOf(c: ContactInfo): number {
  return (
    (c.displayName ? 2 : 0) + (c.displayAddress ? 1 : 0) + (c.organization ? 1 : 0)
  )
}


type PersonRow = {
  id: string
  display_name: string
  location: string | null
  display_name_source: string | null
}

type IdentityRow = {
  person_id: string
  identity_type: string
  identity_value: string
}

type ContactRow = ContactInfo & {
  identityType: string
  normalizedValue: string | null
}

type ContactRowRaw = {
  display_name: string | null
  organization: string | null
  display_address: string | null
  updated_at: string | null
  identity_type: string
  normalized_value: string | null
}

/**
 * Enrich canonical Person display names from Apple Contacts staged identities.
 * Idempotent and replay-safe: only persons whose display_name is an unresolved
 * identity fallback (or not yet marked) are considered.
 */
export async function enrichDisplayNamesFromAppleContacts(): Promise<EnrichResult> {
  const persons = (await sql`
    select id, display_name, location, display_name_source
    from person
    where archived_at is null
  `) as PersonRow[]

  const identities = (await sql`
    select pi.person_id, pi.identity_type, pi.identity_value
    from person_identity pi
    join person p on p.id = pi.person_id
    where p.archived_at is null
  `) as IdentityRow[]

  const rawContacts = (await sql`
    select
      lp.display_name, lp.organization, lp.display_address, lp.updated_at,
      li.identity_type, li.normalized_value
    from l_person lp
    join l_person_identity li on li.l_person_id = lp.id
    where lp.source = 'apple_contacts'
  `) as ContactRowRaw[]

  const contacts: ContactRow[] = rawContacts.map((r) => ({
    displayName: r.display_name,
    organization: r.organization,
    displayAddress: r.display_address,
    updatedAt: r.updated_at,
    identityType: r.identity_type,
    normalizedValue: r.normalized_value,
  }))

  const identitiesByPerson = new Map<string, IdentityRow[]>()
  for (const it of identities) {
    const arr = identitiesByPerson.get(it.person_id) ?? []
    arr.push(it)
    identitiesByPerson.set(it.person_id, arr)
  }

  const contactIndex = buildContactIndex(contacts)

  const enrichedIds: string[] = []
  const enrichedNames: string[] = []
  const enrichedLocs: string[] = []
  const resetIds: string[] = []
  const resetNames: string[] = []
  const markUnresolvedIds: string[] = []
  const resolvedHumanIds: string[] = []
  let ambiguousCount = 0

  for (const person of persons) {
    const personIdents = identitiesByPerson.get(person.id) ?? []

    // APPLE CONTACTS IS THE AUTHORITATIVE SOURCE FOR CLIENT NAMES.
    //
    // There is no in-system screen where an operator enters client details — they are
    // entered in Apple Contacts. So a human name supplied by Contacts must be able to
    // OVERRIDE an inferred or stale one. Without this the first record to claim an
    // identity owns its name forever: on 2026-09-10 a person holding a phone number as
    // "Puerple House" (source_evidence) blocked the human-authored "Juan A. Santa Cruz"
    // on the SAME number, so the client could not be found by name at all.
    //
    // The only name left untouched is one that ALREADY came from Contacts (no churn).
    const keys = personIdents
      .map((it) => identityMatchKey(it.identity_type, it.identity_value))
      .filter((k): k is string => Boolean(k))
    const { contact, ambiguous } = resolveContactForIdentityKeys(keys, contactIndex)
    const contactHumanName =
      contact && isHumanName(contact.displayName) ? contact.displayName : null

    if (contactHumanName && person.display_name_source !== 'apple_contacts') {
      enrichedIds.push(person.id)
      enrichedNames.push(contactHumanName)
      enrichedLocs.push(contact?.displayAddress ?? '')
      continue
    }

    if (isHumanName(person.display_name)) {
      // Already a real name and either already sourced from Contacts or Contacts has no
      // better one. Mark provenance only if it is unmarked.
      if (person.display_name_source === null) resolvedHumanIds.push(person.id)
      continue
    }
    // No reliable single human name. Multiple distinct names -> ambiguous
    // (never guess); no human name -> unmatched. Both stay unresolved.
    if (ambiguous) ambiguousCount += 1

    const fallback = personIdents.map((it) => it.identity_value).find((v) => Boolean(v))
    const isIdentityFallbackName =
      person.display_name.includes('@') || /^[+0-9\s().-]+$/.test(person.display_name)
    if (isIdentityFallbackName) {
      if (person.display_name_source !== 'unresolved') markUnresolvedIds.push(person.id)
    } else if (fallback) {
      resetIds.push(person.id)
      resetNames.push(fallback)
    } else if (person.display_name_source !== 'unresolved') {
      markUnresolvedIds.push(person.id)
    }
  }

  if (enrichedIds.length > 0) {
    await sql`
      update person p set
        display_name = v.display_name,
        display_name_source = 'apple_contacts',
        location = coalesce(p.location, nullif(v.location, ''))
      from (
        select * from unnest(
          ${enrichedIds}::uuid[],
          ${enrichedNames}::text[],
          ${enrichedLocs}::text[]
        ) as t(id, display_name, location)
      ) v
      where p.id = v.id
    `
  }
  if (resetIds.length > 0) {
    await sql`
      update person p set
        display_name = v.name,
        display_name_source = 'unresolved'
      from (
        select * from unnest(${resetIds}::uuid[], ${resetNames}::text[]) as t(id, name)
      ) v
      where p.id = v.id
    `
  }
  if (markUnresolvedIds.length > 0) {
    await sql`
      update person set display_name_source = 'unresolved'
      where id = any(${markUnresolvedIds})
    `
  }
  if (resolvedHumanIds.length > 0) {
    await sql`
      update person set display_name_source = 'source_evidence'
      where id = any(${resolvedHumanIds})
    `
  }

  await refreshClientReadModels()

  return {
    enriched: enrichedIds.length,
    unresolved: resetIds.length + markUnresolvedIds.length,
    ambiguous: ambiguousCount,
    resolvedHuman: resolvedHumanIds.length,
  }
}

