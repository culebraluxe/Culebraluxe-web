// AUTH-09E — DEV-only V1 role assignment for the two known app_users.
// Run: node --env-file=.env.local --import tsx scripts/provision-v1-roles.ts
// Root app_user -> root role; Lisa app_user -> business_power role.
// Idempotent + transactional; removes the redundant legacy `owner` assignment.
import { db } from '../db/client'

const ROOT_USER_ID = '1fc6dc61-d842-4d29-a20b-93c79e07c718'
const LISA_USER_ID = 'aa06d089-162c-4bef-84ec-a76ee38cc8ad'

async function main() {
  const r = await db.transaction('provision-v1-roles', async (tx) => {
    const role = async (code: string) => {
      const rows = await tx`select id from role where code = ${code} and active = true limit 1`
      if (rows.length === 0) throw new Error(`role ${code} missing`)
      return rows[0].id as string
    }
    const rootRole = await role('root')
    const bpRole = await role('business_power')
    const ownerRole = await role('owner')

    // Root user -> root role (drop redundant owner).
    await tx`delete from app_user_role where app_user_id = ${ROOT_USER_ID} and role_id = ${ownerRole}`
    await tx`
      insert into app_user_role (app_user_id, role_id, assigned_by_user_id)
      values (${ROOT_USER_ID}, ${rootRole}, null)
      on conflict (app_user_id, role_id) do nothing
    `
    // Lisa -> business_power (drop redundant owner so she has no settings.manage/tech).
    await tx`delete from app_user_role where app_user_id = ${LISA_USER_ID} and role_id = ${ownerRole}`
    await tx`
      insert into app_user_role (app_user_id, role_id, assigned_by_user_id)
      values (${LISA_USER_ID}, ${bpRole}, null)
      on conflict (app_user_id, role_id) do nothing
    `
  })
  if (!r.ok) {
    console.error('provisioning failed:', r.error)
    process.exit(1)
  }

  const verify = await db.query<{ display: string; role_codes: string[]; authority_codes: string[] }>`
    select u.display_name as display,
      coalesce(jsonb_agg(distinct role.code order by role.code)
        filter (where role.code is not null), '[]'::jsonb) as role_codes,
      coalesce(jsonb_agg(distinct a.code order by a.code)
        filter (where a.code is not null), '[]'::jsonb) as authority_codes
    from app_user u
    left join app_user_role aur on aur.app_user_id = u.id
    left join role on role.id = aur.role_id and role.active = true
    left join role_authority ra on ra.role_id = role.id
    left join authority a on a.id = ra.authority_id
    where u.id in (${ROOT_USER_ID}, ${LISA_USER_ID})
    group by u.id, u.display_name
    order by u.display_name
  `
  console.log('=== V1 ROLE ASSIGNMENTS ===')
  console.log(JSON.stringify(verify.ok ? verify.data : verify.error, null, 2))
  const root = verify.ok ? verify.data.find((x) => x.display === 'CulebraLuxe Root') : undefined
  const lisa = verify.ok ? verify.data.find((x) => x.display === 'Lisa Penfield') : undefined
  const ok =
    root?.role_codes.includes('root') &&
    root?.authority_codes.includes('tech.access') &&
    lisa?.role_codes.includes('business_power') &&
    !lisa?.authority_codes.includes('tech.access') &&
    !lisa?.authority_codes.includes('settings.manage')
  console.log(ok ? 'PROVISION_OK: ROOT(root+tech.access), BUSINESS_POWER(business, no tech)' : 'PROVISION_FAILED')
  process.exit(ok ? 0 : 1)
}
main().catch((e) => {
  console.error('provisioning failed', e)
  process.exit(1)
})
