// ---------------------------------------------------------------------------
// CAPTURE THE DRIVER'S OWN VALUE FORMATS — once, read-only, into a fixture.
//
// The interrupted-sequence proof is env-free (its frozen command loads no env
// file), so it cannot open a connection at test time. This step is the one place
// a real driver is read: it records the exact text Postgres/Neon emits for a
// timestamptz and for numeric columns, so the fixture the proof consumes is the
// driver's own shape rather than a hand-written literal.
//
// READ-ONLY: selects only. It never inserts, updates or deletes a row, so it is
// safe against DEV (APP_ENV=development, the default target for this script).
//
//   APP_ENV=development node --import tsx --env-file=.env.local scripts/capture-driver-value-formats.ts
// ---------------------------------------------------------------------------

import { mkdirSync, writeFileSync } from 'node:fs'
import { dirname, join } from 'node:path'

import { sql } from '@/legacy/db/client'

async function main(): Promise<void> {
  const rows = await sql`
    select
      now()::timestamptz::text as timestamptz_text,
      (extract(epoch from now()) * 1000)::bigint::text as epoch_millis_text,
      (select count(*) from storyboard_story)::text as count_text,
      1234567890123::bigint::text as bigint_text,
      now()::date::text as date_text
  `
  const row = (rows[0] ?? {}) as Record<string, unknown>
  const capture = {
    source: 'postgres/neon tagged executor via db/client sql',
    capturedBy: 'scripts/capture-driver-value-formats.ts',
    note: 'Exact ::text forms the driver emits. Do not hand-edit into a shape the driver never produces.',
    values: {
      timestamptzText: String(row.timestamptz_text),
      epochMillisText: String(row.epoch_millis_text),
      countText: String(row.count_text),
      bigintText: String(row.bigint_text),
      dateText: String(row.date_text),
    },
  }
  const out = join(process.cwd(), 'legacy/workflow_app/tests/fixtures/driver-value-formats.json')
  mkdirSync(dirname(out), { recursive: true })
  writeFileSync(out, `${JSON.stringify(capture, null, 2)}\n`)
  console.log('captured driver value formats:', JSON.stringify(capture.values))
}

main().catch((error) => {
  console.error('capture failed:', error instanceof Error ? error.message : String(error))
  process.exit(1)
})
