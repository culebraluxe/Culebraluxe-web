// ---------------------------------------------------------------------------
// REL-INTEL — Apple Contacts -> neutral evidence projection.
//
// l_person is the CURRENT normalized Apple projection. This module copies that
// current source state into the neutral relationship-evidence store for
// provenance/context only. Identity mastering is performed separately from
// current l_person; evidence is not a promotion queue.
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
  integration_intake_batch_id: string | null
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
 * Project CURRENT l_person rows into relationship evidence. The l_person row's
 * intake batch is preserved so reprojection never clears source provenance.
 */
export async function loadAppleEvidence(execute: QueryExecutor = sql): Promise<number> {
  const rows = (await execute`
    select
      lp.id, lp.integration_intake_batch_id, lp.source_account, lp.source_contact_id,
      lp.display_name, lp.organization, lp.given_name, lp.family_name,
      li.identity_type, li.identity_value, li.normalized_value, li.source_label
    from l_person lp
    left join l_person_identity li on li.l_person_id = lp.id
    where lp.source = ${APPLE_SOURCE}
    order by lp.id, li.ordinal asc, li.id asc
  `) as AppleLoadRow[]

  const byPerson = new Map<string, { person: ApplePersonInput; batchId: string | null }>()
  for (const r of rows) {
    let entry = byPerson.get(r.id)
    if (!entry) {
      const hasPersonName = Boolean(r.given_name?.trim()) || Boolean(r.family_name?.trim())
      const orgOnly = !hasPersonName && Boolean(r.organization?.trim())
      entry = {
        batchId: r.integration_intake_batch_id,
        person: {
          id: r.id,
          sourceAccount: r.source_account,
          sourceContactId: r.source_contact_id,
          displayName: r.display_name,
          organization: r.organization,
          emails: [],
          phones: [],
          isOrganizationOrService: orgOnly,
        },
      }
      byPerson.set(r.id, entry)
    }
    if (r.identity_type === 'email') {
      entry.person.emails.push({
        value: r.identity_value,
        normalized: r.normalized_value ?? r.identity_value,
        label: r.source_label,
      })
    } else if (r.identity_type === 'phone') {
      entry.person.phones.push({
        value: r.identity_value,
        normalized: r.normalized_value ?? r.identity_value,
        label: r.source_label,
      })
    }
  }

  let count = 0
  for (const { person, batchId } of byPerson.values()) {
    const { evidence, fingerprint: fp } = projectApplePersonToEvidence(person)
    await upsertRelationshipEvidence(evidence, fp, batchId ?? undefined, execute)
    count += 1
  }
  return count
}
