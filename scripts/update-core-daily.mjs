// CORE-DAILY — truthful Production Story Board update for the foundation stories.
// Authorized control-plane update. Run:
//   node --env-file=.env.local scripts/update-core-daily.mjs
import { Pool } from '@neondatabase/serverless'
const pool = new Pool({ connectionString: process.env.DATABASE_URL_PROD })

const SHA = 'c4dffcf'
const COMMON = `commit ${SHA} | tsc 0 errors | next build exit 0 | git diff clean`

// id -> { status, completion, note }
const PLAN = {
  'CORE-DAILY-01': {
    status: 'Complete', completion: 100,
    note: 'Extends the existing task model (migration 075 applied+verified in DEV) with a deterministic, auditable, replay-safe relationship follow-up lifecycle: OPEN/SNOOZED/COMPLETED/DISMISSED/CANCELLED + snoozed_until/outcome/next_touch/source/reason/recommendation_key + a command/receipt table. db/follow-up.ts applyFollowUpCommand (create/snooze/complete/dismiss/cancel + done+next-touch) is idempotent via a unique command_id receipt. DEV proof exercised create, replay(no-dup), snooze(sets snoozed_until only, never touches workflow/legal), complete(+completed_at), complete+next-touch(exactly one next task), dismiss, cancel, duplicate(no second effect), 7 receipts persisted. Authorization inherited via portalWrite(crm.write) seam.',
  },
  'CORE-DAILY-02': {
    status: 'Complete', completion: 100,
    note: 'lib/relationship-intel/contact-targets.ts (pure) builds safe native targets (tel/mailto/sms/WhatsApp) with US/PR phone + email validation; WhatsApp only under explicit approval; invalid values omitted honestly; multiple legitimate choices in stable order (Call->Message->Email->WhatsApp). 8 targeted tests pass. Launching never records successful communication (no side effects).',
  },
  'CORE-DAILY-03': {
    status: 'Partial', completion: 30,
    note: 'Follow-up complete command already records outcome + next_touch_at (DEV proven). Full outcome-capture that also writes a truthful Interaction row + completes the originating follow-up via a single composed command is the next bounded step; not yet wired.',
  },
  'CORE-DAILY-04': {
    status: 'Partial', completion: 20,
    note: 'Follow-up create command exists (CORE-DAILY-01). Next-action preset wiring per context (Catch-Up/Client/Contract) + per-context DEV proof pending.',
  },
  'CORE-DAILY-05': {
    status: 'Blocked', completion: 0,
    note: 'Depends on CORE-DAILY-03/04 wiring (contact actions + Done/Snooze + outcome) into /portal/attention plus DEV-backed runtime items. Not yet built.',
  },
  'CORE-DAILY-06': {
    status: 'Blocked', completion: 0,
    note: 'Depends on CORE-DAILY-05 surface. Snooze/Done command mechanics are proven (CORE-DAILY-01) but the UX presets + surface wiring are not yet built.',
  },
  'CORE-DAILY-07': { status: 'Blocked', completion: 0, note: 'Deterministic recommendation projection over REL-INTEL evidence not yet implemented in this session.' },
  'CORE-DAILY-08': { status: 'Blocked', completion: 0, note: 'Depends on CORE-DAILY-07. Suppression/dismissal semantics not yet built.' },
  'CORE-DAILY-09': { status: 'Blocked', completion: 0, note: 'Client immediate-action surface not yet wired (depends on 03/04/05/06/07/08).' },
  'CORE-DAILY-10': { status: 'Blocked', completion: 0, note: 'Contract operating summary not yet implemented.' },
  'CORE-DAILY-11': {
    status: 'Partial', completion: 20,
    note: 'The follow-up command is DEV-proven to mutate only task fields (snooze sets snoozed_until; complete sets completed_at) and never workflow/legal deadlines or Contract stage. Broader workflow/application safety proofs pending.',
  },
  'CORE-DAILY-12': {
    status: 'Partial', completion: 20,
    note: 'REL-INTEL OPPS stewardship seam (link/reject/classify/rerun + audit) exists from REL-INTEL-09; CORE-DAILY-12-specific repair/stewardship proofs against safe DEV data are a follow-on.',
  },
  'CORE-DAILY-13': { status: 'Blocked', completion: 0, note: 'Telemetry infrastructure not yet inspected/wired; blocked pending foundation surfaces.' },
  'CORE-DAILY-14': {
    status: 'Partial', completion: 30,
    note: 'Objective mechanics (44px tap targets in tokens, glass-rail horizontal scroll, no hover-only, build-clean) verified by code; no browser tooling available so viewport rendering is not genuinely proven (Chris QA).',
  },
  'CORE-DAILY-15': { status: 'Blocked', completion: 0, note: 'Integrated verification/release depends on 03-14 completion.' },
}

const rows = await pool.query("select id from storyboard_story where id like 'CORE-DAILY%' order by id")
console.log('CORE-DAILY rows found:', rows.rows.length)
let updated = 0
for (const r of rows.rows) {
  const plan = PLAN[r.id]
  if (!plan) { console.log('SKIP (no plan):', r.id); continue }
  const notes = `CORE-DAILY completion/status evidence (${SHA}): ${plan.note}\n${COMMON}`
  await pool.query(
    'update storyboard_story set status=$1, completion=$2, notes=$3, updated_at=now() where id=$4',
    [plan.status, plan.completion, notes, r.id],
  )
  updated += 1
  console.log('UPDATED:', r.id, '->', plan.status, plan.completion)
}
console.log('UPDATED total:', updated)
await pool.end()
