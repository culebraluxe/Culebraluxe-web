import { sql } from './client'

import { normalizeEmail, normalizePhone } from '@/lib/crm-intake-normalization'

// Read-only identity data-quality projection (CRM-18). Reports deterministic
// gaps in canonical people/identities. No merges, no creation, no fuzzy
// matching, no Apple Contacts. Malformed values are flagged only by applying
// the existing strict email / E.164 phone normalization rules.

export type IdentityTypeCount = { identityType: string; count: number }

export type MalformedIdentity = {
  personId: string
  personName: string
  identityType: string
  value: string
  issue: string
}

export type WeakCoveragePerson = {
  id: string
  displayName: string
  role: string
  status: string
  activeDealCount: number
  openTaskCount: number
}

export type IdentityQualitySnapshot = {
  totalPeople: number
  peopleWithNoIdentity: number
  peopleWithoutEmail: number
  peopleWithoutPrimaryEmail: number
  peopleWithoutPhone: number
  peopleWithoutPrimaryPhone: number
  identityCountByType: IdentityTypeCount[]
  malformedIdentities: MalformedIdentity[]
  weakCoverage: WeakCoveragePerson[]
  exactDuplicateCheck: {
    possible: false
    note: string
  }
}

type PersonSummaryRow = {
  id: string
  display_name: string
  role: string
  status: string
  identity_count: number
  email_count: number
  primary_email_count: number
  phone_count: number
  primary_phone_count: number
  active_deal_count: number
  open_task_count: number
}

type IdentityRow = {
  person_id: string
  person_name: string
  identity_type: string
  identity_value: string
}

export async function getIdentityQuality(): Promise<IdentityQualitySnapshot> {
  const [personRows, typeRows, identityRows] = await Promise.all([
    sql`
      select
        p.id,
        p.display_name,
        p.role,
        p.status,
        count(pi.id)::int as identity_count,
        count(pi.id) filter (where pi.identity_type = 'email') as email_count,
        count(pi.id) filter (where pi.identity_type = 'email' and pi.is_primary) as primary_email_count,
        count(pi.id) filter (where pi.identity_type = 'phone') as phone_count,
        count(pi.id) filter (where pi.identity_type = 'phone' and pi.is_primary) as primary_phone_count,
        (
          select count(*)::int
          from deal d
          where d.client_person_id = p.id and d.stage <> 'closed'
        ) as active_deal_count,
        (
          select count(*)::int
          from task t
          where t.person_id = p.id and t.status = 'open'
        ) as open_task_count
      from person p
      left join person_identity pi
        on pi.person_id = p.id
      where p.archived_at is null
      group by p.id, p.display_name, p.role, p.status
      order by p.display_name asc
    `,
    sql`
      select identity_type, count(*)::int as count
      from person_identity
      group by identity_type
      order by count desc
    `,
    sql`
      select
        pi.person_id,
        p.display_name as person_name,
        pi.identity_type,
        pi.identity_value
      from person_identity pi
      join person p
        on p.id = pi.person_id
      where p.archived_at is null
    `,
  ])

  const people = personRows as PersonSummaryRow[]
  const identities = identityRows as IdentityRow[]

  const malformedIdentities: MalformedIdentity[] = []

  for (const identity of identities) {
    if (identity.identity_type === 'email') {
      try {
        normalizeEmail(identity.identity_value)
      } catch {
        malformedIdentities.push({
          personId: identity.person_id,
          personName: identity.person_name,
          identityType: 'email',
          value: identity.identity_value,
          issue: 'invalid_email',
        })
      }
    } else if (identity.identity_type === 'phone') {
      try {
        normalizePhone(identity.identity_value)
      } catch {
        malformedIdentities.push({
          personId: identity.person_id,
          personName: identity.person_name,
          identityType: 'phone',
          value: identity.identity_value,
          issue: 'invalid_phone',
        })
      }
    }
  }

  const weakCoverage: WeakCoveragePerson[] = people
    .filter(
      (row) =>
        (row.active_deal_count > 0 || row.open_task_count > 0) &&
        row.email_count === 0 &&
        row.phone_count === 0,
    )
    .map((row) => ({
      id: row.id,
      displayName: row.display_name,
      role: row.role,
      status: row.status,
      activeDealCount: row.active_deal_count,
      openTaskCount: row.open_task_count,
    }))

  return {
    totalPeople: people.length,
    peopleWithNoIdentity: people.filter((row) => row.identity_count === 0).length,
    peopleWithoutEmail: people.filter((row) => row.email_count === 0).length,
    peopleWithoutPrimaryEmail: people.filter(
      (row) => row.email_count > 0 && row.primary_email_count === 0,
    ).length,
    peopleWithoutPhone: people.filter((row) => row.phone_count === 0).length,
    peopleWithoutPrimaryPhone: people.filter(
      (row) => row.phone_count > 0 && row.primary_phone_count === 0,
    ).length,
    identityCountByType: (typeRows as { identity_type: string; count: number }[]).map(
      (row) => ({ identityType: String(row.identity_type), count: Number(row.count) }),
    ),
    malformedIdentities,
    weakCoverage,
    exactDuplicateCheck: {
      possible: false,
      note:
        'person_identity enforces UNIQUE (identity_type, identity_value), so exact duplicate identities are structurally impossible; no duplicate detector is provided.',
    },
  }
}
