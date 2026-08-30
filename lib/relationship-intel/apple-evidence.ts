// ---------------------------------------------------------------------------
// REL-INTEL — Apple Contacts -> neutral evidence projection.
//
// The existing Apple staging path (integration_staged_contact_profile -> l_person
// relational-load projection) already holds 2,573 staged contacts. This module
// projects those relational load rows into the SAME neutral evidence seam so
// Apple and Gmail evidence share one read model. It NEVER converts Apple
// contacts into canonical Clients; classification is left to reconciliation.
// ---------------------------------------------------------------------------

import type { QueryExecutor } from '../../db/query-executor'
import { sql } from '../../db/client'
import { upsertRelationshipEvidence } from '../../db/relationship-evidence'
import {
  APPLE_SOURCE,
  projectApplePersonToEvidence,
  type ApplePersonInput,
} from './apple-projector'

type AppleLoadRow = {
  id: string
  source_account: string
  source_contact_id: string
  display_name: string | null
  organization: string | null
  given_name: string | null
  family_name: string | null
  identity_type: string
  identity_value: string
  normalized_value: string | null
  source_label: string | null
}

/**
 * Read the existing l_person relational-load rows and project them into the
 * neutral evidence seam (server-side, replay-safe upsert per contact). A
 * contact with no person name but an organization is an organization/service
 * contact (isOrganizationOrService=true) so it is never promoted to Person.
 */
export async function loadAppleEvidence(execute: QueryExecutor = sql): Promise<number> {
  const rows = (await execute`
    select
      lp.id, lp.source_account, lp.source_contact_id, lp.display_name, lp.organization,
      lp.given_name, lp.family_name,
      li.identity_type, li.identity_value, li.normalized_value, li.source_label
    from l_person lp
    left join l_person_identity li on li.l_person_id = lp.id
    where lp.source = ${APPLE_SOURCE}
    order by lp.id, li.ordinal asc, li.id asc
  `) as AppleLoadRow[]

  const byPerson = new Map<string, ApplePersonInput>()
  for (const r of rows) {
    let person = byPerson.get(r.id)
    if (!person) {
      const hasPersonName =
        Boolean(r.given_name?.trim()) || Boolean(r.family_name?.trim())
      const orgOnly = !hasPersonName && Boolean(r.organization?.trim())
      person = {
        id: r.id,
        sourceAccount: r.source_account,
        sourceContactId: r.source_contact_id,
        displayName: r.display_name,
        organization: r.organization,
        emails: [],
        phones: [],
        isOrganizationOrService: orgOnly,
      }
      byPerson.set(r.id, person)
    }
    if (r.identity_type === 'email') {
      person.emails.push({
        value: r.identity_value,
        normalized: r.normalized_value ?? r.identity_value,
        label: r.source_label,
      })
    } else if (r.identity_type === 'phone') {
      person.phones.push({
        value: r.identity_value,
        normalized: r.normalized_value ?? r.identity_value,
        label: r.source_label,
      })
    }
  }

  let count = 0
  for (const person of byPerson.values()) {
    const { evidence, fingerprint: fp } = projectApplePersonToEvidence(person)
    await upsertRelationshipEvidence(evidence, fp, undefined, execute)
    count += 1
  }
  return count
}
