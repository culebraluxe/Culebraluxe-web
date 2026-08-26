// OPS-11A — DEV issue fixtures. Deterministic, idempotent, and FAILS CLOSED
// outside development/test. Demonstrates RED + YELLOW open and one RESOLVED
// record across at least two domain objects, plus runbook content (driven by
// lib/issue-types.ts config).
//
//  1. refreshes real issues from canonical facts (reconcileIssues)
//  2. inserts a fixed YELLOW fixture + a fixed RESOLVED fixture
//
// Run: node --env-file=.env.local --import tsx scripts/seed-issue-fixtures.ts

import { sql } from '../db/client'
import { reconcileIssues } from '../db/issues'

// Fixed, deterministic fixture ids (never collide with generated uuids).
const FIXTURE_IDS = {
  yellowAppraisal: '11111111-1111-4111-8111-1111111111a1',
  resolvedClosing: '11111111-1111-4111-8111-1111111111a2',
} as const

// Real canonical deals used for fixtures (Brisas del Mar under_contract;
// Casa Solana qualified). Referenced by stable id — never by name/slug.
const DEAL_BRISAS = '60000000-0000-4000-8000-000000000002'
const DEAL_CASA_SOLANA = '60000000-0000-4000-8000-000000000004'

function isDevOrTest(): boolean {
  const env = process.env.APP_ENV ?? 'development'
  return env === 'development' || env === 'test'
}

async function main() {
  if (!isDevOrTest()) {
    console.error('REFUSED: issue fixtures require APP_ENV development or test.')
    process.exit(2)
  }

  // Reset fixtures so re-runs are idempotent.
  await sql`delete from issue where id = ${FIXTURE_IDS.yellowAppraisal} or id = ${FIXTURE_IDS.resolvedClosing}`

  // 1. Real deterministic issues from canonical facts (RED missing-PS on the
  //    under-contract deals). Idempotent + DB deduped.
  const real = await reconcileIssues()

  // 2. Deterministic demo fixtures (fixed ids, fixed domains).
  await sql`
    insert into issue (id, type, severity, state, domain_type, domain_id, title, detail, detected_at)
    values (
      ${FIXTURE_IDS.yellowAppraisal},
      'APPRAISAL_OVERDUE', 'YELLOW', 'OPEN', 'deal', ${DEAL_BRISAS},
      'Appraisal overdue (DEV fixture)',
      'Fixture: appraisal is required and closing is near, but no signed appraisal is on file.',
      now() - interval '2 days'
    )
    on conflict (id) do nothing
  `
  await sql`
    insert into issue (id, type, severity, state, domain_type, domain_id, title, detail, detected_at, resolved_at)
    values (
      ${FIXTURE_IDS.resolvedClosing},
      'CLOSING_DATE_AT_RISK', 'YELLOW', 'RESOLVED', 'deal', ${DEAL_CASA_SOLANA},
      'Closing date at risk (DEV fixture, resolved)',
      'Fixture: demonstrates a resolved issue leaving the open queue.',
      now() - interval '5 days',
      now() - interval '3 days'
    )
    on conflict (id) do nothing
  `

  console.log('reconcile created', real.created, 'resolved', real.resolved, '| open', real.open)
  console.log('seeded YELLOW fixture:', FIXTURE_IDS.yellowAppraisal)
  console.log('seeded RESOLVED fixture:', FIXTURE_IDS.resolvedClosing)
}

main()
  .then(() => process.exit(0))
  .catch((err) => {
    console.error(err)
    process.exit(1)
  })
