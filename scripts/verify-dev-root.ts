// AUTH-09C — DEV verification for the root Google subject.
// Run: node --env-file=.env.local --import tsx scripts/verify-dev-root.ts
import { db } from '../db/client'
const SUBJECT = '104033509608344385707'
const EMAIL = 'culebraluxe@gmail.com'

async function main() {
  const identity = await db.query<{
    provider: string
    provider_subject: string
    provider_email: string | null
    app_user_id: string
  }>`
    select ai.provider, ai.provider_subject, ai.provider_email, ai.app_user_id
    from auth_identity ai
    where ai.provider = 'google' and ai.provider_subject = ${SUBJECT}
  `
  console.log('=== auth_identity for google/' + SUBJECT + ' ===')
  console.log(JSON.stringify(identity.ok ? identity.data : identity.error, null, 2))

  const users = await db.query<{
    id: string
    display_name: string
    email: string | null
    account_type: string
    active: boolean
  }>`
    select u.id, u.display_name, u.email, u.account_type, u.active
    from app_user u where u.email = ${EMAIL}
  `
  console.log('=== app_user(s) for culebraluxe@gmail.com ===')
  console.log(JSON.stringify(users.ok ? users.data : users.error, null, 2))

  const authority = await db.query<{ role_codes: string[]; authority_codes: string[] }>`
    select
      coalesce(jsonb_agg(distinct r.code order by r.code)
        filter (where r.code is not null), '[]'::jsonb) as role_codes,
      coalesce(jsonb_agg(distinct a.code order by a.code)
        filter (where a.code is not null), '[]'::jsonb) as authority_codes
    from app_user u
    join auth_identity ai on ai.app_user_id = u.id
    left join app_user_role aur on aur.app_user_id = u.id
    left join role r on r.id = aur.role_id and r.active = true
    left join role_authority ra on ra.role_id = r.id
    left join authority a on a.id = ra.authority_id
    where ai.provider = 'google' and ai.provider_subject = ${SUBJECT}
    group by u.id
  `
  console.log('=== roles + authorities for the mapped app_user ===')
  console.log(JSON.stringify(authority.ok ? authority.data : authority.error, null, 2))

  const dup = await db.query<{ n: number }>`
    select count(*)::int as n from auth_identity
    where provider = 'google' and provider_subject = ${SUBJECT}
  `
  const idRow = identity.ok && identity.data.length === 1 ? identity.data[0] : null
  const userRows = users.ok ? users.data : []
  const authRow = authority.ok ? authority.data[0] : null
  const dupN = dup.ok && dup.data.length === 1 ? (dup.data[0].n as number) : 0
  const checks = {
    exactly_one_identity: identity.ok === true && identity.data.length === 1,
    maps_to_canonical_root: idRow != null && userRows.some((u) => u.id === idRow.app_user_id),
    subject_is_numeric_stable: /^[0-9]+$/.test(SUBJECT) === true && !SUBJECT.includes('-'),
    no_email_identity_key: idRow?.provider_email == null,
    exactly_one_intended_app_user: userRows.length === 1,
    app_user_active_internal: userRows.length === 1 && userRows[0].active === true && userRows[0].account_type === 'internal',
    owner_role_present: authRow?.role_codes.includes('owner') === true,
    portal_read_effective: authRow?.authority_codes.includes('portal.read') === true,
    no_duplicate_or_conflicting_identity: dupN === 1,
  }
  console.log('=== CHECKLIST ===')
  for (const [k, v] of Object.entries(checks)) console.log(`${v ? 'PASS' : 'FAIL'}  ${k}`)
  const allPass = Object.values(checks).every(Boolean)
  console.log(allPass ? 'VERIFY_OK' : 'VERIFY_FAILED')
  process.exit(allPass ? 0 : 1)
}
main().catch((e) => {
  console.error('verification failed', e)
  process.exit(1)
})
