// AUTH-08G — DEV-only explicit provisioning: Google subject -> auth_identity ->
// the canonical existing app_user, and grant the seeded 'owner' role so the
// account holds portal.read.
//
// Run: node --env-file=.env.local --import tsx scripts/provision-dev-google-identity.ts <google_sub>
//
// This is EXPLICIT provisioning, NOT auto-creation / email fallback / bypass:
//   - it never creates an app_user (the canonical internal admin must exist)
//   - it only inserts auth_identity (provider, provider_subject) against that
//     existing app_user, plus the existing seeded 'owner' role assignment
//   - provider_email is informational only and left NULL (never a lookup key)
//   - guarded + idempotent (ON CONFLICT DO NOTHING)
// The canonical app_user for the current administrator:
const ADMIN_APP_USER_ID = 'aa06d089-162c-4bef-84ec-a76ee38cc8ad'

import { db } from '../db/client'

async function main() {
  const subject = process.argv[2]
  if (!subject) {
    console.error('usage: provision-dev-google-identity.ts <google_sub>')
    process.exit(1)
  }
  const trimmed = subject.trim()
  if (!/^[A-Za-z0-9._-]+$/.test(trimmed)) {
    console.error('refusing: subject must be a stable non-email identifier')
    process.exit(1)
  }
  if (trimmed.length > 255) {
    console.error('refusing: subject too long')
    process.exit(1)
  }

  const target = await db.queryOne<{
    id: string
    display_name: string
    account_type: string
    active: boolean
  }>`
    select id, display_name, account_type, active
    from app_user
    where id = ${ADMIN_APP_USER_ID}
    limit 1
  `
  if (!target.ok) {
    console.error('DB failure while locating target app_user', target.error)
    process.exit(1)
  }
  if (!target.data) {
    console.error(`refusing: canonical app_user ${ADMIN_APP_USER_ID} not found`)
    process.exit(1)
  }
  if (target.data.account_type !== 'internal' || target.data.active !== true) {
    console.error('refusing: target app_user must be internal AND active')
    process.exit(1)
  }

  const result = await db.transaction('provision-google-identity', async (tx) => {
    // Guard: subject must not already be mapped to a DIFFERENT app_user.
    const conflict = await tx`
      select app_user_id from auth_identity
      where provider = 'google' and provider_subject = ${trimmed}
      limit 1
    `
    if (conflict.length > 0 && conflict[0].app_user_id !== ADMIN_APP_USER_ID) {
      throw new Error('refusing: google subject already mapped to another app_user')
    }

    await tx`
      insert into auth_identity (app_user_id, provider, provider_subject, provider_email)
      values (${ADMIN_APP_USER_ID}, 'google', ${trimmed}, null)
      on conflict (provider, provider_subject) do nothing
    `

    const ownerRole = await tx`
      select id from role where code = 'owner' and active = true limit 1
    `
    if (ownerRole.length === 0) {
      throw new Error('owner role missing; run migration 015')
    }
    await tx`
      insert into app_user_role (app_user_id, role_id, assigned_by_user_id)
      values (${ADMIN_APP_USER_ID}, ${ownerRole[0].id}, null)
      on conflict (app_user_id, role_id) do nothing
    `
  })
  if (!result.ok) {
    console.error('provisioning failed:', result.error)
    process.exit(1)
  }

  // Verify: mapped + active + owner + portal.read present.
  const verify = await db.query<{
    display_name: string
    role_codes: string[]
    authority_codes: string[]
  }>`
    select
      u.display_name,
      coalesce(
        jsonb_agg(distinct r.code order by r.code)
          filter (where r.code is not null), '[]'::jsonb
      ) as role_codes,
      coalesce(
        jsonb_agg(distinct a.code order by a.code)
          filter (where a.code is not null), '[]'::jsonb
      ) as authority_codes
    from app_user u
    left join auth_identity ai on ai.app_user_id = u.id
    left join app_user_role aur on aur.app_user_id = u.id
    left join role r on r.id = aur.role_id and r.active = true
    left join role_authority ra on ra.role_id = r.id
    left join authority a on a.id = ra.authority_id
    where u.id = ${ADMIN_APP_USER_ID}
    group by u.id, u.display_name
  `
  console.log('=== PROVISIONED (app_user view) ===')
  console.log(verify.ok ? JSON.stringify(verify.data, null, 2) : verify.error)
  if (
    !verify.ok ||
    !verify.data[0]?.role_codes.includes('owner') ||
    !verify.data[0]?.authority_codes.includes('portal.read')
  ) {
    console.error('verification FAILED: owner/portal.read not present')
    process.exit(1)
  }
  console.log('PROVISION_OK: google subject mapped to owner with portal.read')
}

main().then(
  () => process.exit(0),
  (e) => {
    console.error('provisioning failed', e)
    process.exit(1)
  },
)
