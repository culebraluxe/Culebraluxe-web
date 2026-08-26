// AUTH-09B — DEV-only provisioning: root user (culebraluxe@gmail.com) -> auth_identity
// -> app_user -> owner role (portal.read).
//
// Run AFTER capturing the STABLE Google subject from a post-fix login:
//   node --env-file=.env.local --import tsx scripts/provision-dev-root.ts <stable_google_sub>
//
// Guards:
//   - REJECTS UUID-format subjects (a real Google `sub` is numeric). If the
//     captured value is still a UUID, the canonical-subject fix did NOT take;
//     do NOT provision (stop and fix the identity layer).
//   - does not duplicate an existing culebraluxe@gmail.com app_user
//   - explicit auth_identity insert (provider=google, subject, app_user_id)
//   - assigns the highest existing role (owner), which grants portal.read
//   - idempotent (ON CONFLICT DO NOTHING)
//   - provider_email informational only (NULL here; never an identity key)
const ROOT_EMAIL = 'culebraluxe@gmail.com'
const ROOT_DISPLAY = 'CulebraLuxe Root'

import { db } from '../db/client'

async function main() {
  const subject = process.argv[2]
  if (!subject) {
    console.error('usage: provision-dev-root.ts <stable_google_sub>')
    process.exit(1)
  }
  const s = subject.trim()
  // Real Google `sub` values are numeric. A UUID here means the identity source
  // is still the per-login randomUUID (fix not effective) — refuse loudly.
  if (/[a-fA-F0-9]{8}-[a-fA-F0-9]{4}/.test(s) || !/^[0-9]+$/.test(s)) {
    console.error(`REFUSE: subject "${s}" is not a numeric Google sub (fix not effective).`)
    process.exit(1)
  }

  const result = await db.transaction('provision-dev-root', async (tx) => {
    // Guard: subject must not already map to a DIFFERENT app_user.
    const conflict = await tx`
      select app_user_id from auth_identity
      where provider = 'google' and provider_subject = ${s}
      limit 1
    `

    // Find or create the root app_user (no duplicate).
    const existing = await tx`
      select id from app_user where email = ${ROOT_EMAIL} limit 1
    `
    let rootUserId: string
    if (existing.length > 0) {
      rootUserId = existing[0].id as string
    } else {
      const created = await tx`
        insert into app_user (display_name, email, account_type, active)
        values (${ROOT_DISPLAY}, ${ROOT_EMAIL}, 'internal', true)
        returning id
      `
      if (created.length === 0) throw new Error('failed to create root app_user')
      rootUserId = created[0].id as string
    }

    if (conflict.length > 0 && conflict[0].app_user_id !== rootUserId) {
      throw new Error('refuse: google subject already mapped to another app_user')
    }

    await tx`
      insert into auth_identity (app_user_id, provider, provider_subject, provider_email)
      values (${rootUserId}, 'google', ${s}, null)
      on conflict (provider, provider_subject) do nothing
    `

    const owner = await tx`
      select id from role where code = 'owner' and active = true limit 1
    `
    if (owner.length === 0) throw new Error('owner role missing; run migration 015')
    await tx`
      insert into app_user_role (app_user_id, role_id, assigned_by_user_id)
      values (${rootUserId}, ${owner[0].id}, null)
      on conflict (app_user_id, role_id) do nothing
    `
    return rootUserId
  })
  if (!result.ok) {
    console.error('provisioning failed:', result.error)
    process.exit(1)
  }
  const rootUserId = result.data

  // Verify: mapped + active/internal + owner + portal.read.
  const verify = await db.query<{
    display_name: string
    email: string | null
    account_type: string
    active: boolean
    role_codes: string[]
    authority_codes: string[]
  }>`
    select
      u.display_name, u.email, u.account_type, u.active,
      coalesce(jsonb_agg(distinct r.code order by r.code)
        filter (where r.code is not null), '[]'::jsonb) as role_codes,
      coalesce(jsonb_agg(distinct a.code order by a.code)
        filter (where a.code is not null), '[]'::jsonb) as authority_codes
    from app_user u
    left join auth_identity ai on ai.app_user_id = u.id and ai.provider = 'google'
    left join app_user_role aur on aur.app_user_id = u.id
    left join role r on r.id = aur.role_id and r.active = true
    left join role_authority ra on ra.role_id = r.id
    left join authority a on a.id = ra.authority_id
    where u.id = ${rootUserId}
    group by u.id, u.display_name, u.email, u.account_type, u.active
  `
  console.log('=== ROOT PROVISIONED ===')
  console.log(JSON.stringify(verify.ok ? verify.data : verify.error, null, 2))
  const row = verify.ok ? verify.data[0] : undefined
  const ok =
    row &&
    row.active === true &&
    row.account_type === 'internal' &&
    row.role_codes.includes('owner') &&
    row.authority_codes.includes('portal.read')
  console.log(ok ? 'PROVISION_OK: root mapped to owner with portal.read' : 'PROVISION_FAILED')
  process.exit(ok ? 0 : 1)
}

main().catch((e) => {
  console.error('provisioning failed', e)
  process.exit(1)
})
