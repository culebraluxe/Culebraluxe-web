#!/usr/bin/env node
// ---------------------------------------------------------------------------
// A HUMAN passed a gate.
//
// The captain is the human verifier (Forge has always had human limits — that is
// why the Workflow Engine is here). Visual/iPad/keyboard acceptance cannot be
// machine-verified in this repo, so the human walks it and records the pass here.
//
// Usage:
//   node --env-file=.env.local scripts/forge-human-gate-pass.mjs \
//     --story PROJECTS-WORKSPACE-18 --gate "iPad touch targets" \
//     --note "walked on iPad 11in; drawers transfer focus; targets >=48px" [--target prod|dev|both]
//
// Writes: a durable marker + timestamp into the story's notes (never touching
// acceptance_criteria), so the pass is auditable and cannot be confused with a
// machine result. Prints the resulting story state.
// ---------------------------------------------------------------------------

import { Pool } from '@neondatabase/serverless'

const argv = process.argv.slice(2)
const arg = (name) => {
  const i = argv.indexOf(`--${name}`)
  return i >= 0 ? argv[i + 1] : null
}

const story = arg('story')
const gate = arg('gate')
const note = arg('note')
const target = arg('target') ?? 'both'
if (!story || !gate) {
  console.error('usage: forge-human-gate-pass --story <id> --gate "<what>" [--note "<evidence>"] [--target prod|dev|both]')
  process.exit(1)
}

const targets = []
if (target === 'prod' || target === 'both') targets.push(['PROD', process.env.DATABASE_URL_PROD])
if (target === 'dev' || target === 'both') targets.push(['DEV', process.env.DATABASE_URL_DEV])

const stamp = new Date().toISOString()
const marker = `HUMAN GATE PASS 2026-09-10 (human-verified, NOT machine-verified): ${gate}${note ? ` — ${note}` : ''} [${stamp}]`

for (const [label, url] of targets) {
  const pool = new Pool({ connectionString: url })
  const found = await pool.query(`select id, notes from storyboard_story where id = $1`, [story])
  if (found.rowCount === 0) {
    console.log(`${label}: ${story} not found — skipped`)
  } else {
    const notes = String(found.rows[0].notes ?? '')
    if (notes.includes(marker)) {
      console.log(`${label}: identical pass already recorded — skipped`)
    } else {
      await pool.query(
        `update storyboard_story set notes = coalesce(notes,'') || $2, updated_at = now() where id = $1`,
        [story, `\n\n${marker}`],
      )
      console.log(`${label}: recorded human pass for ${story}: ${gate}`)
    }
  }
  await pool.end()
}
