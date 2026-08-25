// ---------------------------------------------------------------------------
// REL-INTEL — DB-backed reconciliation lookup.
//
// Uses the existing canonical identity seam (db/person-identities.findIdentityMatch)
// so reconciliation NEVER reads or writes canonical tables directly beyond a
// read-only exact-identity match. Canonical person_identity is UNIQUE on
// (identity_type, identity_value), so each normalized email/phone matches at
// most one Person — ambiguity is handled at the evidence level (one source
// contact holding multiple identities pointing at different people).
// ---------------------------------------------------------------------------

import type { QueryExecutor } from '../../db/query-executor'
import { sql } from '../../db/client'
import type { NormalizedIdentityHint } from '../crm-intake-types'
import { findIdentityMatch } from '../../db/person-identities'
import type { PersonLookup } from './reconcile'

export function createDbPersonLookup(execute?: QueryExecutor): PersonLookup {
  return {
    // A human-approved promotion is the only "source link": evidence rows whose
    // canonical link was recorded via the command/receipt seam.
    findExplicitSourceLink: async (source, sourceAccount, sourceIdentityKey) => {
      const q = execute ?? sql
      const rows = (await q`
        select canonical_person_id
        from integration_relationship_evidence
        where source = ${source}
          and source_account = ${sourceAccount}
          and source_identity_key = ${sourceIdentityKey}
          and canonical_person_id is not null
        limit 1
      `) as { canonical_person_id: string | null }[]
      return rows[0]?.canonical_person_id
        ? { personId: String(rows[0].canonical_person_id) }
        : null
    },

    findPeopleByEmail: async (normalizedEmail) => {
      const hint: NormalizedIdentityHint = {
        kind: 'email',
        value: normalizedEmail,
        normalizedValue: normalizedEmail,
        evidence: 'user_supplied',
      }
      const match = await findIdentityMatch(hint, execute)
      return match ? [{ personId: match.personId }] : []
    },

    findPeopleByPhone: async (normalizedPhone) => {
      const hint: NormalizedIdentityHint = {
        kind: 'phone',
        value: normalizedPhone,
        normalizedValue: normalizedPhone,
        evidence: 'user_supplied',
      }
      const match = await findIdentityMatch(hint, execute)
      return match ? [{ personId: match.personId }] : []
    },
  }
}
