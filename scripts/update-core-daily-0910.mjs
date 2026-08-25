// CORE-DAILY-09 + 10 — Production Story Board completion update.
// Run: node --env-file=.env.local scripts/update-core-daily-0910.mjs
import { Pool } from '@neondatabase/serverless'
const pool = new Pool({ connectionString: process.env.DATABASE_URL_PROD })
const SHA = 'd5d3797'
const PLAN = {
  'CORE-DAILY-09': {
    status: 'Complete', completion: 100,
    note: `Client Manager detail now renders an "Act" panel (ClientDailyActions) with native contact actions, outcome capture (recordOutcomeAction), and next-action presets (createQuickNextActionAction). DEV proof (${SHA}): outcome -> canonical Interaction written; replay no-dup (1 row); quick next action -> follow-up created (open); contact evidence present from person_identity. No forced navigation into Dossier for routine work.`,
  },
  'CORE-DAILY-10': {
    status: 'Complete', completion: 100,
    note: `Deal workspace now renders a compact "Operating summary" panel from authoritative Deal facts: current gate/stage, next action (first open task), offer state, closing/material dates, client, property. DEV proof (${SHA}): stage=showing, property=Casa Luar, client=Ana Rivera, next action present, offers counted; quick action from Contract creates a follow-up linked to the deal. No duplicate transaction state; mutations stay on canonical seams.`,
  },
}
for (const [id, plan] of Object.entries(PLAN)) {
  const notes = `CORE-DAILY ${id} evidence (${SHA}): ${plan.note}`
  await pool.query('update storyboard_story set status=$1, completion=$2, notes=$3, updated_at=now() where id=$4', [plan.status, plan.completion, notes, id])
  console.log('UPDATED:', id, '->', plan.status, plan.completion)
}
await pool.end()
