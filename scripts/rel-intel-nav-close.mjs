// REL-INTEL final visual closure — durable Production Story Board records.
// Authorized control-plane update:
//   1) ARCH-HANDOFF: dated superseding note for the Workflow/Forms nav decision.
//   2) REL-INTEL-07: record the browser-verification blocker (stays Partial/90).
import { Pool } from '@neondatabase/serverless'
const pool = new Pool({ connectionString: process.env.DATABASE_URL_PROD })

const SUPERSEDE =
  '\n\n[2026-08-25 CTO/Product Owner] SUPERSEDES the earlier continuity note that ' +
  '"Workflows and Forms remain functional but hidden from visible CORE navigation." ' +
  'Workflows (/portal/workflows) and Forms (/portal/forms) are now visible CORE ' +
  'secondary-navigation destinations, reusing their existing screens/routes. ' +
  'Navigation-only restoration; no screens rebuilt, no routes changed.'

const BLOCKER =
  '\n\n[2026-08-25] Browser/mobile visual verification was NOT performed: no browser ' +
  'tooling (Playwright/Puppeteer) is available in the implementation environment. ' +
  'Catch-Up relationship context and the restored Workflow/Forms nav are code-, ' +
  'test-, and build-verified, but iPhone/iPad/desktop viewport rendering is not ' +
  'genuinely proven. REL-INTEL-07 remains Partial/90 (mobile-readability acceptance ' +
  'unverified); pending Chris visual QA.'

try {
  const ah = await pool.query(
    "select notes from storyboard_story where id = 'ARCH-HANDOFF'",
  )
  if (ah.rows[0]) {
    await pool.query(
      "update storyboard_story set notes = $1, updated_at = now() where id = 'ARCH-HANDOFF'",
      [ah.rows[0].notes + SUPERSEDE],
    )
    console.log('ARCH-HANDOFF: superseding note appended')
  } else {
    console.log('ARCH-HANDOFF: row not found')
  }

  const r7 = await pool.query("select notes from storyboard_story where id = 'REL-INTEL-07'")
  if (r7.rows[0]) {
    await pool.query(
      "update storyboard_story set notes = $1, updated_at = now() where id = 'REL-INTEL-07'",
      [r7.rows[0].notes + BLOCKER],
    )
    console.log('REL-INTEL-07: blocker note appended (stays Partial/90)')
  } else {
    console.log('REL-INTEL-07: row not found')
  }
} catch (e) { console.error('ERR', e.message); process.exitCode = 1 } finally { await pool.end() }
