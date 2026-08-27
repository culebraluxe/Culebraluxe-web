// ---------------------------------------------------------------------------
// CATCH-UP — DEV-only deterministic task fixtures (workstream tree proof).
//
// Uses the canonical public.task table — never a UI-only mock array. Inserts a
// small representative set across the five workstreams using FIXED UUIDs so
// provisioning is idempotent and cleanup is exact. Real application tasks are
// never touched (this script only ever manages its own namespaced ids).
//
// Prereq: run db/migrations/086_task_taxonomy.sql first (adds task.workstream,
// task.category, and removes the all-or-nothing task_has_context check).
//
// Usage:
//   APP_ENV=development node --env-file=.env.local --import tsx \
//     scripts/provision-catchup-task-fixtures.ts            # provision / upsert
//   APP_ENV=development node --env-file=.env.local --import tsx \
//     scripts/provision-catchup-task-fixtures.ts --cleanup  # remove exactly these
// ---------------------------------------------------------------------------

import { sql } from '../db/client'

const APP_ENV = (process.env.APP_ENV ?? 'development').trim().toLowerCase()
const IS_DEV = APP_ENV === 'development' || APP_ENV === 'dev' || APP_ENV === 'test'

type Fixture = {
  id: string
  workstream: string
  category: string
  title: string
}

// Fixed, deterministic ids (v4-shaped but static) so re-runs are safe and the
// --cleanup path deletes exactly what this script owns.
const FIXTURES: Fixture[] = [
  { id: 'a1000000-0000-4000-8000-000000000001', workstream: 'CLIENT', category: 'FOLLOWUP', title: 'Call Ana Rivera' },
  { id: 'a1000000-0000-4000-8000-000000000002', workstream: 'CLIENT', category: 'ONBOARDING', title: 'Enter DR property data' },
  { id: 'a1000000-0000-4000-8000-000000000003', workstream: 'CLIENT', category: 'CONTRACTS', title: 'Listing Agreement — Greece' },
  { id: 'a1000000-0000-4000-8000-000000000004', workstream: 'CLIENT', category: 'MEDIA', title: 'DR photography package' },
  { id: 'a1000000-0000-4000-8000-000000000005', workstream: 'CORE', category: 'ACCOUNTING', title: 'Onboard accountant' },
  { id: 'a1000000-0000-4000-8000-000000000006', workstream: 'CORE', category: 'LEGAL', title: 'Get PR Tax EIN' },
  { id: 'a1000000-0000-4000-8000-000000000007', workstream: 'CORE', category: 'MANAGEMENT', title: 'Get business cards' },
  { id: 'a1000000-0000-4000-8000-000000000008', workstream: 'OPPS', category: 'DATA_ENTRY', title: 'Reconcile listing intake data' },
  { id: 'a1000000-0000-4000-8000-000000000009', workstream: 'SUPPORT', category: 'LIGHTS_ON', title: 'Verify operating services' },
  { id: 'a1000000-0000-4000-8000-00000000000a', workstream: 'SUPPORT', category: 'BACKUPS', title: 'Verify backup cycle' },
  { id: 'a1000000-0000-4000-8000-00000000000b', workstream: 'TECH', category: 'NEW_TECH', title: 'Finish Forms' },
  { id: 'a1000000-0000-4000-8000-00000000000c', workstream: 'TECH', category: 'INFRASTRUCTURE', title: 'Harden production environment' },
]

const FIXTURE_IDS = FIXTURES.map((f) => f.id)

async function provision(): Promise<void> {
  for (const f of FIXTURES) {
    await sql`
      insert into task (id, title, workstream, category, task_kind, priority, status)
      values (${f.id}, ${f.title}, ${f.workstream}, ${f.category}, 'human', 0, 'open')
      on conflict (id) do update set
        title = excluded.title,
        workstream = excluded.workstream,
        category = excluded.category,
        status = 'open',
        updated_at = now()
    `
  }
  console.log(`provisioned ${FIXTURES.length} Catch-Up task fixtures (DEV)`)
}

async function cleanup(): Promise<void> {
  const result = (await sql`
    delete from task where id = any (${FIXTURE_IDS})
    returning id
  `) as { id: string }[]
  console.log(`cleaned ${result.length} Catch-Up task fixtures (DEV)`)
}

async function verify(): Promise<void> {
  const rows = (await sql`
    select workstream, category, title, status
    from task
    where id = any (${FIXTURE_IDS})
    order by workstream asc, category asc, title asc
  `) as Array<{
    workstream: string | null
    category: string | null
    title: string
    status: string
  }>
  if (rows.length === 0) {
    console.log('no fixtures present (run without --cleanup to provision)')
    return
  }
  console.log(`verified ${rows.length} Catch-Up task fixtures in public.task:\n`)
  let lastWs: string | null = null
  for (const r of rows) {
    if (r.workstream !== lastWs) {
      console.log(`\n${r.workstream}`)
      lastWs = r.workstream
    }
    console.log(`  ${r.category ?? ''}   ${r.title}   [${r.status}]`)
  }
  console.log('')
}

async function main(): Promise<void> {
  if (!IS_DEV) {
    console.error(
      `refusing: fixtures are DEV-only; APP_ENV=${JSON.stringify(process.env.APP_ENV)}`,
    )
    process.exit(2)
  }

  const cleanupOnly = process.argv.includes('--cleanup')
  const verifyOnly = process.argv.includes('--verify')
  if (verifyOnly) await verify()
  else if (cleanupOnly) await cleanup()
  else await provision()
  process.exit(0)
}

void main()
