// AUTH-08H — DEV-only verification of the Google identity provisioning.
// Run: node --env-file=.env.local --import tsx scripts/verify-dev-google-provision.ts
// Reads ONLY the DEV database. Confirms the step-4 required proof facts.
import { db } from '../db/client'

const SUBJECT = '9a5682f4-6531-4a86-9282-ad80bba996f6'
const APP_USER_ID = 'aa06d089-162c-4bef-84ec-a76ee38cc8ad'

async function main() {
  // 1. Exactly one auth_identity row for this google subject, mapping to the
  //    canonical app_user.
  const ai = await db.query<{
    provider: string
    provider_subject: string
    app_user_id: string
    provider_email: string | null
    display_name: string
  }>`
    select ai.provider, ai.provider_subject, ai.app_user_id, ai.provider_email,
           u.display_name
    from auth_identity ai
    join app_user u on u.id = ai.app_user_id
    where ai.provider = 'google' and ai.provider_subject = ${SUBJECT}
  `
  console.log('=== auth_identity for subject ===')
  console.log(JSON.stringify(ai.ok ? ai.data : ai.error, null, 2))

  // 2. app_user is active + internal (no duplicate app_user for the admin).
  const user = await db.query<{
    id: string
    display_name: string
    account_type: string
    active: boolean
  }>`
    select id, display_name, account_type, active
    from app_user
    where email = 'lisa@culebraluxe.com'
    order by created_at
  `
  console.log('=== app_user(s) for lisa@culebraluxe.com (expect exactly 1, active, internal) ===')
  console.log(JSON.stringify(user.ok ? user.data : user.error, null, 2))

  // 3. owner role assigned + grants portal.read.
  const authority = await db.query<{
    role_codes: string[]
    authority_codes: string[]
  }>`
    select
      coalesce(jsonb_agg(distinct r.code order by r.code)
        filter (where r.code is not null), '[]'::jsonb) as role_codes,
      coalesce(jsonb_agg(distinct a.code order by a.code)
        filter (where a.code is not null), '[]'::jsonb) as authority_codes
    from app_user u
    left join app_user_role aur on aur.app_user_id = u.id
    left join role r on r.id = aur.role_id and r.active = true
    left join role_authority ra on ra.role_id = r.id
    left join authority a on a.id = ra.authority_id
    where u.id = ${APP_USER_ID}
    group by u.id
  `
  console.log('=== roles + authorities for canonical app_user ===')
  console.log(JSON.stringify(authority.ok ? authority.data : authority.error, null, 2))

  // 4. No duplicate/conflicting auth_identity for this subject or provider.
  const dup = await db.query`
    select provider_subject, count(*)::int as n
    from auth_identity
    where provider = 'google' and provider_subject = ${SUBJECT}
    group by provider_subject
  `
  console.log('=== duplicate auth_identity check (expect n=1) ===')
  console.log(JSON.stringify(dup.ok ? dup.data : dup.error, null, 2))

  // Assemble PASS/FAIL.
  const aiRow = ai.ok && ai.data.length === 1 ? ai.data[0] : null
  const userRow = user.ok && user.data.length === 1 ? user.data[0] : null
  const authRow = authority.ok ? authority.data[0] : null
  const dupRow = dup.ok && dup.data.length === 1 ? dup.data[0] : null
  const checks = {
    exactly_one_identity: aiRow !== null,
    maps_to_canonical_app_user: aiRow?.app_user_id === APP_USER_ID,
    provider_email_not_identity_key: aiRow?.provider_email === null,
    app_user_active_internal:
      userRow?.active === true && userRow?.account_type === 'internal',
    no_duplicate_app_user: (user.ok ? user.data.length : 0) === 1,
    owner_role_assigned: authRow?.role_codes.includes('owner') === true,
    owner_grants_portal_read: authRow?.authority_codes.includes('portal.read') === true,
    no_duplicate_or_conflicting_identity: dupRow?.n === 1,
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
