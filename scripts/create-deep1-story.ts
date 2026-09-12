// ONE-OFF: create the DEEP1 reference story on the Story Board — the durable handoff
// note for future sessions, alongside ARCH-HANDOFF (ARCH1) and SOP1.
//
//   node --env-file=.env.local --import tsx scripts/create-deep1-story.ts
//
// Idempotent: an existing DEEP1 is reported and left untouched.
//
// NOTE: this script deliberately uses an explicit PROD pool rather than the
// application gateway. db/database-gateway.ts binds its executor at module load, so a
// script that flips APP_ENV in its own body still connects to the environment that was
// set when the gateway was imported. See DEEP1 section 6 — that trap is part of why
// this note exists.
import { forgeDb, forgeDbTargetForUrl } from '../db/forge-db'

import { createPoolExecutor } from './lib/pool-executor'
import { createStoryboardStory, type StoryboardStoryInput } from '../db/storyboard'
import { getStoryboardStory } from '../db/storyboard'

const STORY_ID = 'DEEP1'

const STORY: StoryboardStoryInput = {
  id: STORY_ID,
  workstream: 'ARCH',
  operatingSurface: 'TECH',
  title: 'DEEP1 — Data Pipeline Doctrine: ODS -> Warehouse -> Screen (READ SECOND)',
  priority: 'Reference',
  status: 'Complete',
  batch: 0,
  completion: 100,
  rollup: false,
  goal:
    'Restore a future session to the data-pipeline doctrine and the traps that cost real hours, without reconstructing prior conversation.',
  scope:
    'Reference-only handoff. Not executable backlog, must not create an agent work item. Keep rollup=false, like ARCH-HANDOFF and SOP1.',
  preconditions:
    'Read ARCH-HANDOFF (ARCH1) first for architecture and the operating model, then this for the data pipeline, then observe fresh runtime state because live facts advance after any note is written.',
  postconditions:
    'A future session can state the ODS/warehouse/screen rule, the warehouse grain, the round-trip cost law, and the database-target trap without re-deriving them.',
  contextRefs:
    'READ SECOND; data pipeline; ODS; warehouse; screen contract; source grain; DEEP1; continuity record',
  acceptanceCriteria:
    'A future session reads this and does not (a) let application code read an l_ table, (b) invent a second channel vocabulary, (c) write one interaction row per event, (d) loop one query per row, or (e) trust a resolved-target diagnostic without printing the connection host.',
  dependencies: null,
  architectBrief: null,
  plannedStartAt: null,
  actualStartAt: null,
  completedAt: null,
  // NOTES is defined below (it is long); main() merges it in so the object literal
  // stays readable.
  notes: '',
}

const NOTES = `Durable data-pipeline doctrine and session handoff. NON-ROLLUP reference story (same shape as ARCH-HANDOFF and SOP1): not executable backlog, must not create an agent work item, keep rollup=false.

[1] THE THREE LAYERS, AND THE RULE THAT MATTERS MOST
ODS is the l_* tables: raw intake, written by the intake scripts, and NOTHING client-facing may ever read one. L is write-only. It keeps everything, and that is precisely what makes the warehouse safe to be lossy — every warehouse row is re-derivable from L. The WAREHOUSE holds only what the screen contract needs, cherry-picked out of ODS by promotion scripts, which are the ONLY code permitted to read an l_ table. The SCREEN reads the warehouse through a service (services/*), never through a repository inline.

[2] THE SCREEN IS THE CONTRACT
Read the pane before inventing a mapping. components/portal/contact-history.tsx declares TWO vocabularies that are NOT interchangeable: SOURCE_SLOTS (six source rows — Phone, iMessage, WhatsApp, Email, FaceTime, Apple Calendar) and channelMeta (moment rows — call, email, imessage, sms, whatsapp, meeting, showing, note). Phone deliberately EXCLUDES FaceTime, because a call and a FaceTime are different things. lib/relationship-intel/channels.ts is the single implementation; do not re-derive a channel map anywhere else.

[3] WAREHOUSE GRAIN
interaction holds ONE row per Person x source carrying the NEWEST event — not one row per event. Measured: Ami went 5,519 rows to 5 (one per source); the table went 51,013 to 1,705. Writers use upsertLatestInteraction keyed latest:<person_id>:<channel> with "where interaction.occurred_at < excluded.occurred_at", so a re-run UPDATES rather than appends and an older event can never overwrite a newer one. Migrations 164/165 did the in-place collapse AND re-keyed the survivors: collapsing without re-keying leaves the survivor on its old identity, so the next sync INSERTS a duplicate instead of updating it.

[4] ROUND-TRIP COST IS THE DEFAULT BUG
Measured on this Mac over the pooled Neon driver: 68.7ms per round trip. One set-based statement over 20,000 rows: 71ms. So a per-row loop is the fault, not the database — the iMessage materializer was 186,000 sequential round trips (roughly 5 hours) for work the database finishes in seconds. Batch with unnest and chunk at about 500 rows.

[5] IDENTITY TRAPS THAT COST HOURS
- Apple Mail: the RFC Message-ID lives in message_global_data.message_id_header (with angle brackets). messages.message_id is an INTEGER and is NEVER an identity; using it double-lands every message because Mail's AppleEvent bridge reports the RFC for the same message.
- Phone vs FaceTime: both store channel 'call' in interaction. The delineation is source_system (apple_calls | apple_facetime) or event_type (phone_call | facetime_call) — the intake already records it. Read it back; never merge the two.
- The Contact History MV deliberately filters the neutral 'Attachment'/'Message' placeholders, so a newest message that happens to be an attachment shows no preview even though earlier messages had text. Outstanding refinement: take the preview from the newest message that actually has text.

[6] THE DATABASE-TARGET TRAP (this one silently wrote to the wrong branch)
db/database-gateway.ts does "const SQL_EXECUTOR = createExecutorSafely()" at MODULE LOAD, so the Neon executor is built from whatever APP_ENV is present when that module is first imported. A script that sets process.env.APP_ENV in its own body does so AFTER its static imports have already run, so the connection stays bound to .env.local's development target — while dbTargetInfo() still reports target "prod", because it re-reads process.env at call time. This actually happened: a seed run printed "resolved target: prod" and wrote to DEV. RULE: set APP_ENV before importing any db module (dynamic import inside main), assert the resolved target equals the requested target, and ALWAYS confirm by printing the connection HOST, because the Neon branch token is only visible there. .env.local also stores APP_ENV quoted ("development"), so compare against 'production' on the unquoted value.

[7] MIGRATIONS AND READ MODELS
Apply with scripts/apply-migration.mjs <file> dev, then prod; the schema_migration ledger records target, checksum and note. A materialized view does NOT see a DELETE against its base table — after any data change, refresh mv_client_relationship_channels, mv_client_directory and mv_client_contact_history explicitly or the screen serves stale rows.

[8] WHERE THINGS STOOD WHEN THIS WAS WRITTEN (2026-09-11/12)
Intake: mail reads the local Mail.app Envelope Index in discrete bands (0-1, 1-3, 3-6, 6-12 months, MAIL_SYNC_BAND), not AppleEvents — one property read cost up to ~700ms on the big mailbox. iMessage and calls come from the Apple export. Warehouse grain is done for apple_messages, apple_calls and apple_facetime; icloud_mail (email) still writes one interaction per message and needs the same two-line change. services/comms owns the Person-to-screen contract for comms. Contact History is a glance panel; its per-message archive was removed deliberately. PROD had ZERO projects until jessica-iverson-listing was seeded (scripts/seed-jessica-project.ts, anchored to person b741d639-3173-47bc-adff-769865c6347d). PROJECTS-WORKSPACE-13..18 remain Planned: 16 (deterministic DEV fixtures covering row-anchored, WBS-entity-anchored and unanchored) is the fixture story and is SEPARATE from that seeded row; 14 (real-project acceptance) is additionally blocked by TECH-DEBT-07 (releaseEvidence never populated) and by the contract domain being unreleased in PROD.`

async function main() {
  const url = process.env.DATABASE_URL_PROD
  if (!url) throw new Error('DATABASE_URL_PROD is not set (fail closed)')
  if (url === process.env.DATABASE_URL_DEV) throw new Error('PROD URL equals DEV URL (fail closed)')
  // The branch token lives in the HOST, so print it: a resolved-target diagnostic alone
  // has already lied once in this repo (see DEEP1 section 6).
  console.log(`target host: ${new URL(url).host}`)

  const pool = createPoolExecutor(url)
  try {
    const existing = await getStoryboardStory(STORY_ID, pool.execute)
    if (existing) {
      console.log(`${STORY_ID} already exists ("${existing.title}", status ${existing.status}) - left untouched.`)
      return
    }
    const created = await createStoryboardStory({ ...STORY, notes: NOTES }, pool.execute)
    console.log(`created ${created.id}: ${created.title}`)
    console.log(`  workstream=${created.workstream} surface=${created.operatingSurface} status=${created.status} priority=${created.priority}`)
    console.log(`  notes length: ${created.notes.length} chars`)
  } finally {
    await pool.end()
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})

