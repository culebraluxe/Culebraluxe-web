// AUTH-08G — DEV-only inspection of the identity/authorization tables.
// Run: node --env-file=.env.local --import tsx scripts/inspect-dev-identity.ts
// Reads ONLY the DEV database (APP_ENV=development -> DATABASE_URL_DEV).
import { db } from '../db/client'

async function main() {
  const users = await db.query<{
    id: string
    display_name: string
    email: string | null
    account_type: string
    active: boolean
    person_id: string | null
  }>`
    select id, display_name, email, account_type, active, person_id
    from app_user
    order by display_name
  `
  console.log('=== APP_USERS ===')
  console.log(users.ok ? JSON.stringify(users.data, null, 2) : users.error)

  const roles = await db.query<{
    id: string
    display_name: string
    account_type: string
    active: boolean
    role_codes: string[]
  }>`
    select u.id, u.display_name, u.account_type, u.active,
      coalesce(
        jsonb_agg(r.code order by r.code)
          filter (where r.code is not null),
        '[]'::jsonb
      ) as role_codes
    from app_user u
    left join app_user_role aur on aur.app_user_id = u.id
    left join role r on r.id = aur.role_id and r.active = true
    group by u.id, u.display_name, u.account_type, u.active
    order by u.display_name
  `
  console.log('=== USER ROLES (active) ===')
  console.log(roles.ok ? JSON.stringify(roles.data, null, 2) : roles.error)

  const ai = await db.query<{
    provider: string
    provider_subject: string
    provider_email: string | null
    display_name: string
  }>`
    select ai.provider, ai.provider_subject, ai.provider_email, u.display_name
    from auth_identity ai
    join app_user u on u.id = ai.app_user_id
    order by ai.provider, ai.provider_subject
  `
  console.log('=== AUTH_IDENTITY ===')
  console.log(ai.ok ? JSON.stringify(ai.data, null, 2) : ai.error)
}

main().then(
  () => process.exit(0),
  (e) => {
    console.error('inspection failed', e)
    process.exit(1)
  },
)
