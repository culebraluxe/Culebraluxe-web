// ---------------------------------------------------------------------------
// REL-INTEL — in-memory reconciliation lookup + bounded concurrency helper.
//
// Canonical identity ownership comes from person_identity. Durable source
// ownership comes from integration_source_person_link once migration 097 is
// present. Before that migration is applied, the read path falls back to legacy
// evidence links so deployment ordering cannot break reconciliation reads.
// ---------------------------------------------------------------------------
import type { QueryExecutor } from '../../db/query-executor'
import type { PersonLookup } from './reconcile'

export async function mapLimit<T, R>(
  items: T[],
  limit: number,
  fn: (item: T, index: number) => Promise<R>,
): Promise<R[]> {
  const results = new Array<R>(items.length)
  let cursor = 0
  async function worker() {
    while (cursor < items.length) {
      const idx = cursor
      cursor += 1
      results[idx] = await fn(items[idx], idx)
    }
  }
  const workers = Array.from({ length: Math.min(limit, items.length) }, worker)
  await Promise.all(workers)
  return results
}

export function phoneDigitsKey(value: string): string {
  return value.replace(/[^0-9]/g, '')
}

export function emailKey(value: string): string {
  return value.trim().toLowerCase()
}

/** Preload canonical identity ownership + durable source ownership once. */
export async function createInMemoryPersonLookup(
  execute: QueryExecutor,
): Promise<{
  lookup: PersonLookup
  emailToPerson: Map<string, string[]>
  phoneToPerson: Map<string, string[]>
}> {
  const identityRows = (await execute`
    select pi.identity_type, pi.identity_value, pi.person_id
    from person_identity pi
    join person p on p.id = pi.person_id
    where p.archived_at is null
  `) as { identity_type: string; identity_value: string; person_id: string }[]

  const emailToPerson = new Map<string, string[]>()
  const phoneToPerson = new Map<string, string[]>()
  for (const r of identityRows) {
    if (r.identity_type === 'email') {
      const key = emailKey(r.identity_value)
      const owners = emailToPerson.get(key) ?? []
      if (!owners.includes(r.person_id)) owners.push(r.person_id)
      emailToPerson.set(key, owners)
    } else if (r.identity_type === 'phone') {
      const key = phoneDigitsKey(r.identity_value)
      const owners = phoneToPerson.get(key) ?? []
      if (!owners.includes(r.person_id)) owners.push(r.person_id)
      phoneToPerson.set(key, owners)
    }
  }

  const linkTable = (await execute`
    select to_regclass('public.integration_source_person_link')::text as name
  `) as { name: string | null }[]
  const hasDedicatedLinks = Boolean(linkTable[0]?.name)

  const links = (hasDedicatedLinks
    ? await execute`
        select source, source_account, source_identity_key, canonical_person_id
        from integration_source_person_link
      `
    : await execute`
        select source, source_account, source_identity_key, canonical_person_id
        from integration_relationship_evidence
        where canonical_person_id is not null
      `) as {
    source: string
    source_account: string
    source_identity_key: string
    canonical_person_id: string
  }[]

  const linkMap = new Map<string, string>()
  for (const l of links) {
    linkMap.set(`${l.source}\u0000${l.source_account}\u0000${l.source_identity_key}`, l.canonical_person_id)
  }

  const lookup: PersonLookup = {
    findExplicitSourceLink: async (source, sourceAccount, sourceIdentityKey) => {
      const pid = linkMap.get(`${source}\u0000${sourceAccount}\u0000${sourceIdentityKey}`)
      return pid ? { personId: pid } : null
    },
    findPeopleByEmail: async (normalizedEmail) =>
      (emailToPerson.get(emailKey(normalizedEmail)) ?? []).map((personId) => ({ personId })),
    findPeopleByPhone: async (normalizedPhone) =>
      (phoneToPerson.get(phoneDigitsKey(normalizedPhone)) ?? []).map((personId) => ({ personId })),
  }

  return { lookup, emailToPerson, phoneToPerson }
}
