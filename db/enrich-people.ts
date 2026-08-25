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

export type ContactInfo = {
  displayName: string | null
  organization: string | null
  displayAddress: string | null
}

export type EnrichResult = {
  enriched: number
  unresolved: number
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
 * Build an identity -> Apple Contacts staged record index. When the same
 * identity appears in several Contacts, prefer the one carrying a human name.
 */
export function buildContactIndex(
  contacts: Array<
    ContactInfo & { identityType: string; normalizedValue: string | null }
  >,
): Map<string, ContactInfo> {
  const index = new Map<string, ContactInfo>()
  for (const c of contacts) {
    const key = identityMatchKey(c.identityType, c.normalizedValue)
    if (!key) continue
    const info: ContactInfo = {
      displayName: c.displayName,
      organization: c.organization,
      displayAddress: c.displayAddress,
    }
    const existing = index.get(key)
    if (!existing || (!isHumanName(existing.displayName) && isHumanName(info.displayName))) {
      index.set(key, info)
    }
  }
  return index
}

/** Find the best Apple Contacts record for a set of normalized identity keys. */
export function findContactForIdentityKeys(
  keys: string[],
  index: Map<string, ContactInfo>,
): ContactInfo | null {
  let best: ContactInfo | null = null
  for (const key of keys) {
    const info = index.get(key)
    if (!info) continue
    if (isHumanName(info.displayName)) return info
    if (!best) best = info
  }
  return best
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
      lp.display_name, lp.organization, lp.display_address,
      li.identity_type, li.normalized_value
    from l_person lp
    join l_person_identity li on li.l_person_id = lp.id
    where lp.source = 'apple_contacts'
  `) as ContactRowRaw[]

  const contacts: ContactRow[] = rawContacts.map((r) => ({
    displayName: r.display_name,
    organization: r.organization,
    displayAddress: r.display_address,
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

  for (const person of persons) {
    const personIdents = identitiesByPerson.get(person.id) ?? []

    if (isHumanName(person.display_name)) {
      // Already a real name. Mark provenance only if it is unmarked.
      if (person.display_name_source === null) resolvedHumanIds.push(person.id)
      continue
    }

    // Identity fallback / non-human label (phone, email, or a structured ID) —
    // try to resolve a trusted human name from Apple Contacts.
    const keys = personIdents
      .map((it) => identityMatchKey(it.identity_type, it.identity_value))
      .filter((k): k is string => Boolean(k))
    const contact = findContactForIdentityKeys(keys, contactIndex)

    if (contact && isHumanName(contact.displayName)) {
      enrichedIds.push(person.id)
      enrichedNames.push(contact.displayName as string)
      enrichedLocs.push(contact.displayAddress ?? '')
      continue
    }

    // No trusted human name. CORE RULE: IDENTITY IS NOT DISPLAY NAME. The
    // identity string must not be presented as a good client name. If the
    // current display_name is already an identity fallback (phone/email), just
    // ensure it is marked unresolved. If it is a non-human, non-phone label
    // (e.g. a structured ABPerson ID), revert it to the identity fallback.
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

  return {
    enriched: enrichedIds.length,
    unresolved: resetIds.length + markUnresolvedIds.length,
    resolvedHuman: resolvedHumanIds.length,
  }
}

