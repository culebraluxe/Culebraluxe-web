// ---------------------------------------------------------------------------
// REL-INTEL — in-memory reconciliation lookup + bounded concurrency helper.
//
// Equivalent to createDbPersonLookup but preloads the canonical identity seam
// (person_identity) and prior explicit source links once into memory, so
// reconciliation passes avoid thousands of per-row round-trips. Reads are still
// read-only over the canonical tables; it never writes them. Used by DEV load
// tooling and the OPPS "rerun reconciliation" stewardship path.
// ---------------------------------------------------------------------------
import type { QueryExecutor } from '../../db/query-executor'
import type { PersonLookup } from './reconcile'

/** Run async work with a concurrency limit (bounded parallel round-trips). */
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

/** Preload canonical person_identity (email/phone) + prior explicit source links. */
export async function createInMemoryPersonLookup(
  execute: QueryExecutor,
): Promise<{
  lookup: PersonLookup
  emailToPerson: Map<string, string>
  phoneToPerson: Map<string, string>
}> {
  const identityRows = (await execute`
    select pi.identity_type, pi.identity_value, pi.person_id
    from person_identity pi
    join person p on p.id = pi.person_id
    where p.archived_at is null
  `) as { identity_type: string; identity_value: string; person_id: string }[]

  const emailToPerson = new Map<string, string>()
  const phoneToPerson = new Map<string, string>()
  for (const r of identityRows) {
    const target = r.identity_type === 'email' ? emailToPerson : phoneToPerson
    if (!target.has(r.identity_value)) target.set(r.identity_value, r.person_id)
  }

  const links = (await execute`
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
      emailToPerson.has(normalizedEmail) ? [{ personId: emailToPerson.get(normalizedEmail)! }] : [],
    findPeopleByPhone: async (normalizedPhone) =>
      phoneToPerson.has(normalizedPhone) ? [{ personId: phoneToPerson.get(normalizedPhone)! }] : [],
  }

  return { lookup, emailToPerson, phoneToPerson }
}
