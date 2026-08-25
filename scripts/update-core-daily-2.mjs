// CORE-DAILY — truthful Production Story Board update (session 2).
// Run: node --env-file=.env.local scripts/update-core-daily-2.mjs
import { Pool } from '@neondatabase/serverless'
const pool = new Pool({ connectionString: process.env.DATABASE_URL_PROD })

const SHA = '81f0dd0' // filled after commit
const COMMON = `session-2 commit ${SHA} | migrations 075/076/077 applied+proven in DEV | tsc clean | next build exit 0 | git diff clean`

const PLAN = {
  'CORE-DAILY-03': { status: 'Complete', completion: 100, note: 'recordContactOutcome writes a canonical Interaction (channel/event_type/direction outbound/source_system relationship_follow_up/source_external_id=commandId) + completes the originating follow-up (source_interaction_id correlation) + optional replay-safe next touch. DEV proof: connected/no_answer/next-touch, replay no-dup Interaction, exactly one next obligation.' },
  'CORE-DAILY-04': { status: 'Complete', completion: 100, note: 'Reusable next-action presets (call_back, send_information, schedule_showing, prepare_offer, check_financing, follow_up_lawyer, check_appraisal, check_inspection, check_closing_readiness, custom) + canonical follow-up create command. DEV proof: next action from Person and Deal context, replay no-dup.' },
  'CORE-DAILY-05': { status: 'Complete', completion: 100, note: '/portal/attention follow-up queue now renders native contact actions (ContactActions) + Done/outcome/Snooze/next-action (FollowUpActions) for each real DEV-backed item; why/due/person/deal context preserved; no fake UI-only state. Browser visual QA remains Chris.' },
  'CORE-DAILY-06': { status: 'Complete', completion: 100, note: 'Snooze presets (later today/tomorrow/next week) + Done/Done+next touch wired via the CORE-DAILY-01 lifecycle. DEV proof: snoozed item leaves active queue, returns when due, completed item removed, exactly one next obligation.' },
  'CORE-DAILY-07': { status: 'Complete', completion: 100, note: 'Deterministic explainable recommendation projection (overdue, due-soon, unanswered inbound, two-way no-next-step, quiet past client) with reason + explanationCode + evidencePointers. 8 pure tests + DEV proof (overdue appears). No opaque scoring.' },
  'CORE-DAILY-08': { status: 'Complete', completion: 100, note: 'Idempotent dismissal (relationship_recommendation_dismissal, unique person+code) keeps a recommendation suppressed across regeneration; overdue dismissal no longer falls through to due-soon. DEV proof: dismissed stays suppressed, regeneration count 0.' },
  'CORE-DAILY-09': { status: 'Partial', completion: 30, note: 'Reusable contact + next-action seams (ContactActions/FollowUpActions, canonical outcome/follow-up commands) are ready and proven. Client Manager surface wiring (relationship memory + immediate action) is the remaining work.' },
  'CORE-DAILY-10': { status: 'Partial', completion: 10, note: 'Authoritative-fact contract surface not yet implemented; depends on Deal read model. No duplicate transaction state created.' },
  'CORE-DAILY-11': { status: 'Complete', completion: 100, note: 'Negative proof: daily relationship commands mutate only task fields; DEV before/after proves deal.stage and workflow process_instances unchanged; no direct workflow/contract mutation.' },
  'CORE-DAILY-12': { status: 'Complete', completion: 100, note: 'Reuses REL-INTEL OPPS seam: link (exact_linked/source_link) and reject (rejected) on integration_relationship_evidence; DEV proof of link/reject/no silent Person merge.' },
  'CORE-DAILY-13': { status: 'Complete', completion: 100, note: 'daily_loop_telemetry (migration 077) + best-effort emit for outcome_recorded, followup_completed/snoozed, next_touch_created. No private content stored; DEV proof of rows with empty-ish metadata.' },
  'CORE-DAILY-14': { status: 'Partial', completion: 70, note: 'Objective mechanics: 44px tap targets in tokens, glass-rail horizontal scroll, no hover-only, aria-labels on contact actions, min-h-11 buttons, build-clean. No browser tooling; fine-grained visual QA remains Chris.' },
  'CORE-DAILY-15': { status: 'Complete', completion: 100, note: 'Integrated DEV proof: Catch-Up overdue item -> contact outcome -> canonical Interaction -> Done+next -> Snooze next -> Client context; end-to-end chain proven and cleaned up.' },
}

const rows = await pool.query("select id from storyboard_story where id like 'CORE-DAILY%' order by id")
for (const r of rows.rows) {
  const plan = PLAN[r.id]
  if (!plan) continue
  const notes = `CORE-DAILY ${r.id} evidence (${SHA}): ${plan.note}\n${COMMON}`
  await pool.query('update storyboard_story set status=$1, completion=$2, notes=$3, updated_at=now() where id=$4', [plan.status, plan.completion, notes, r.id])
  console.log('UPDATED:', r.id, '->', plan.status, plan.completion)
}
await pool.end()
