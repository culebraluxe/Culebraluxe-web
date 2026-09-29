import { test } from 'node:test'
import assert from 'node:assert/strict'

import { forgeDb, forgeDbTargetForUrl } from '@/legacy/db/forge-db'

/**
 * THE FENCE AROUND A LOCKOUT, not a description of one.
 *
 * The role-administration screen assigns CANONICAL roles to users. Authorities are the older permission model,
 * and they are still the thing that lets someone into the portal at all: `app/portal/layout.tsx` requires
 * `portal.read` and redirects to `/login/unauthorized` without it.
 *
 * Measured in DEV on 2026-09-24, before migration 214: `business_power_user` — the canonical role an operator
 * would move a user onto — held ZERO authorities while the legacy `business_power` it replaces held six. So the
 * act of "tidying up" a user's role removed their portal access. Nothing failed loudly; the user was simply
 * redirected to an unauthorized page, which reads like a session problem rather than a role one.
 *
 * The rule this enforces: a canonical role must never grant LESS than the legacy role it replaces. Two ways to
 * satisfy it, and either is fine — carry the authorities forward (what 214 does) or drop the legacy code. What is
 * not fine is shipping a canonical role that is quietly narrower.
 *
 * A database that cannot be reached FAILS rather than skips, matching `column-writer-live-schema.test.ts`: a
 * silent pass is how a lockout ships. This file is on demand (`pnpm test:app:db`).
 */
const REPLACEMENTS: Record<string, string[]> = {
  business_power_user: ['business_power', 'bus_power_user', 'agent'],
  user: ['ops', 'viewer'],
  internal_guest: ['guest', 'client'],
  owner: [],
  root: [],
}

/** Roles the admin screen offers, per the canonical set in the DAO's target_role list. */
const ASSIGNABLE = ['internal_guest', 'user', 'business_power_user', 'owner', 'root']

async function liveAuthorities(): Promise<Map<string, Set<string>>> {
  const db = forgeDb.forTarget(forgeDbTargetForUrl(process.env.DATABASE_URL_DEV))
  try {
    const rows = (
      await db.query(`
        select r.code,
               coalesce(array_agg(a.code order by a.code) filter (where a.code is not null), '{}') as codes
          from security_role r
          left join role_authority ra on ra.role_id = r.id
          left join authority a on a.id = ra.authority_id
         where r.account_type = 'internal' and r.active
         group by r.code
      `)
    ).rows as { code: string; codes: string[] }[]
    return new Map(rows.map((row) => [row.code, new Set(row.codes)]))
  } finally {
    await db.end()
  }
}

test('no canonical role grants less than the legacy role it replaces', async () => {
  const held = await liveAuthorities()

  for (const [canonical, legacyCodes] of Object.entries(REPLACEMENTS)) {
    const mine = held.get(canonical)
    assert.ok(mine, `canonical role ${canonical} is missing from security_role`)

    for (const legacy of legacyCodes) {
      const theirs = held.get(legacy)
      if (!theirs) continue // a legacy code that no longer exists grants nothing and replaces nothing
      const missing = [...theirs].filter((code) => !mine.has(code))
      assert.deepEqual(
        missing,
        [],
        `${canonical} is narrower than ${legacy}: moving a user onto it would remove ${missing.join(', ')}`,
      )
    }
  }
})

test('every assignable role can enter the portal', async () => {
  const held = await liveAuthorities()

  for (const role of ASSIGNABLE) {
    const mine = held.get(role)
    assert.ok(mine, `assignable role ${role} is missing from security_role`)
    assert.ok(
      mine.has('portal.read'),
      `${role} can be assigned but lacks portal.read, so assigning it locks the user out of the portal`,
    )
  }
})
