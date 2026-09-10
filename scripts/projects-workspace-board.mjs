// ---------------------------------------------------------------------------
// PROJECTS-WORKSPACE board — additions, amendments, and the DEV mirror.
//
// GPT posted PROJECTS-WORKSPACE-00..14 to the PROD Story Board. Review found:
//   * three GAPS no story covered (error capture, deterministic acceptance
//     fixtures, read-model scale guardrails) → stories 15/16/17;
//   * three STALE/UNDERSPECIFIED points (anchor provenance already shipped by
//     ENG-PROJECTS-ANCHOR-01/02; document provenance; WS-14, which cannot be
//     RECORDED complete while deploy receipts are unfilled and must state
//     behaviour when the unreleased `contract` domain is absent);
//   * one BLOCKER for running Forge: the worker runs with APP_ENV=development, so
//     the PROD-only stories are invisible to it → mirror them into DEV.
//
// Idempotent: amendments append only when the marker is absent; upserts key on id.
// Dry-run by default; pass --apply to write.
// ---------------------------------------------------------------------------

import { Pool } from '@neondatabase/serverless'

const MARK = 'AMENDED 2026-09-10'
const APPLY = process.argv.includes('--apply')
const STORY_IDS = 'PROJECTS-WORKSPACE-%'

const NEW_STORIES = [
  {
    id: 'PROJECTS-WORKSPACE-15',
    title: 'Durable error capture and observability for the Projects seams',
    priority: 'High',
    batch: 4,
    goal:
      'Every failing Projects server seam is captured durably through the canonical error framework — never a silent 500/fallback and never a console-only error.',
    scope:
      'ui/projects server loaders and projections, app/portal/projects route handlers and server actions, and the repositories/services they call.',
    acceptance_criteria: [
      'A forced database failure in the Projects loader produces a durable app_error row via a canonical seam (withApiHandler / withServerErrorCapture / captureServerError, or the DB gateway / ServiceErrorSink) carrying route or label plus correlation.',
      'Expected business outcomes (validation failures, FORBIDDEN, not-found) are audited control flow and produce NO error rows.',
      'No new bare try/catch that swallows, and no console.error-only failure path, in the touched seams.',
      'The UI shows WS-01 error states; a captured failure never renders invented data as if it were true.',
      'scripts/probe-error-capture.ts stays green and the captured row is visible in the TECH view.',
    ].join('\n'),
    dependencies: 'PROJECTS-WORKSPACE-01',
    notes:
      'BASELINE STATUS: MISSING. None of the baseline 15 names the Error Capture Obligation that AGENTS.md makes a review reject for new server code. Additive only; no schema change.',
    test_mode: 'SCOPED',
    assay_commands: [
      '- `pnpm exec tsc --noEmit`',
      '- `node --env-file=.env.local --import tsx scripts/probe-error-capture.ts`',
    ].join('\n'),
  },
  {
    id: 'PROJECTS-WORKSPACE-16',
    title: 'Canonical Projects acceptance fixtures (deterministic, no listing special-casing)',
    priority: 'High',
    batch: 4,
    goal:
      'DEV and PROD acceptance runs exercise a deterministic canonical Projects dataset covering all three anchor provenances, without special-casing any listing in application code.',
    scope:
      'A db/seeds fixture plus an idempotent apply/verify script, and the projection tests that assert the three provenance cases.',
    acceptance_criteria: [
      'A fresh DEV receives the fixture idempotently (re-running the apply changes nothing).',
      'All three provenance cases are provable end to end from the fixture: row-anchored, WBS-entity-anchored, and unanchored.',
      'Application code contains no listing-specific branching — no listing name (including Casa Luar) appears in ui/projects, services, or repositories.',
      'The shipped effective-anchor projection (row > wbs > none) is exercised, not re-implemented.',
    ].join('\n'),
    dependencies: 'PROJECTS-WORKSPACE-01',
    notes:
      'BASELINE STATUS: PARTIAL. Casa Luar is the canonical real fixture, but the shipped anchor provenance needs deterministic data to prove all three cases. Application code must never special-case it (AGENTS).',
    test_mode: 'SCOPED',
    assay_commands: [
      '- `pnpm exec tsx --test testv2/projects-service-projection.test.ts`',
      '- `pnpm exec tsc --noEmit`',
    ].join('\n'),
  },
  {
    id: 'PROJECTS-WORKSPACE-17',
    title: 'Projects read-model performance and scale guardrails',
    priority: 'Medium',
    batch: 4,
    goal:
      'The workspace stays bounded and responsive as WBS items, documents and activity grow, with verified supporting indexes and no per-row query fan-out.',
    scope:
      'The server composition path behind the workspace, index verification on the anchor columns, and a large-fixture regression test.',
    acceptance_criteria: [
      'Composing the workspace for a 500-work-item project stays within a stated bound and shows no query-count growth proportional to row count per pane.',
      'Supporting indexes on the anchor columns are verified present in DEV and PROD.',
      'A regression test over a synthetic large fixture fails if per-row queries are introduced.',
      'Searchable business facts stay in explicit relational fields; no client-side join compensates for a missing server field.',
    ].join('\n'),
    dependencies: 'PROJECTS-WORKSPACE-01',
    notes:
      'BASELINE STATUS: MISSING. Nothing in the baseline bounds query cost for a large project; AGENTS requires searchable facts in explicit relational fields rather than client joins.',
    test_mode: 'SCOPED',
    assay_commands: [
      '- `pnpm exec tsx --test testv2/projects-service-projection.test.ts`',
      '- `pnpm exec tsc --noEmit`',
    ].join('\n'),
  },
]

const AMENDMENTS = [
  {
    id: 'PROJECTS-WORKSPACE-01',
    field: 'acceptance_criteria',
    append:
      `\n\n${MARK}: reuse the SHIPPED effective-anchor projection in ui/projects/service-projection.ts ` +
      "(anchorSource 'row' | 'wbs' | 'none', precedence row > wbs > none) delivered by ENG-PROJECTS-ANCHOR-01/02 " +
      '— extend it, do not rebuild it. Repository boundaries own driver-value normalization (timestamps, counts, ' +
      'jsonb) before values leave the repository; do not patch UI code to compensate for unnormalized driver types.',
  },
  {
    id: 'PROJECTS-WORKSPACE-10',
    field: 'acceptance_criteria',
    append:
      `\n\n${MARK}: document provenance must CONSUME the shipped anchorSource rather than recomputing anchors, ` +
      'and an unanchored project must be explained explicitly — never rendered with invented documents.',
  },
  {
    id: 'PROJECTS-WORKSPACE-14',
    field: 'acceptance_criteria',
    append:
      `\n\n${MARK}: two constraints must be explicit. ` +
      '(1) DEPLOY RECEIPTS — a PROD rollout cannot be RECORDED as deployment-verified while ' +
      'AgentRunEvidence.releaseEvidence is never populated (MEMORY 2026-09-10). DEV acceptance may complete; ' +
      'the PROD/verified half is blocked until the deploy-receipt story lands or the captain explicitly waives it. ' +
      'Do NOT fabricate a receipt to satisfy this story. ' +
      "(2) CONTRACT DOMAIN — the contract domain is not released in PROD; the 'only a correct Listing Agreement' " +
      'acceptance must state how the workspace behaves when contract is absent instead of requiring an unreleased domain.',
  },
]

async function board(url) {
  return new Pool({ connectionString: url })
}

async function main() {
  const prod = await board(process.env.DATABASE_URL_PROD)
  const dev = await board(process.env.DATABASE_URL_DEV)

  const sample = await prod.query(
    `select workstream, operating_surface from storyboard_story where id = 'PROJECTS-WORKSPACE-01'`,
  )
  const workstream = sample.rows[0]?.workstream ?? 'ENGINEERING'
  const surface = sample.rows[0]?.operating_surface ?? 'NEXUS'
  console.log(`plan: workstream=${JSON.stringify(workstream)} surface=${surface} apply=${APPLY}`)

  // ---- additions -----------------------------------------------------------
  for (const s of NEW_STORIES) {
    console.log(`[add] ${s.id} — ${s.title}`)
    if (!APPLY) continue
    await prod.query(
      `insert into storyboard_story
         (id, workstream, title, priority, status, notes, batch, goal, scope, acceptance_criteria,
          dependencies, preconditions, postconditions, operating_surface, test_mode, assay_commands,
          completion, rollup)
       values ($1,$2,$3,$4,'Planned',$5,$6,$7,$8,$9,$10,null,null,$11,$12,$13,0,false)
       on conflict (id) do update set
         title = excluded.title, priority = excluded.priority, batch = excluded.batch,
         goal = excluded.goal, scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
         dependencies = excluded.dependencies, notes = excluded.notes,
         operating_surface = excluded.operating_surface, test_mode = excluded.test_mode,
         assay_commands = excluded.assay_commands, updated_at = now()`,
      [s.id, workstream, s.title, s.priority, s.notes, s.batch, s.goal, s.scope, s.acceptance_criteria,
        s.dependencies, surface, s.test_mode, s.assay_commands],
    )
  }

  // ---- amendments (append-only, marker-guarded) -----------------------------
  for (const a of AMENDMENTS) {
    const cur = await prod.query(`select ${a.field} as v from storyboard_story where id = $1`, [a.id])
    const existing = cur.rows[0]?.v ?? ''
    if (existing.includes(MARK)) {
      console.log(`[amend] ${a.id} already carries ${MARK} — skipped`)
      continue
    }
    console.log(`[amend] ${a.id}.${a.field} += 1 note`)
    if (!APPLY) continue
    await prod.query(
      `update storyboard_story set ${a.field} = coalesce(${a.field},'') || $2, updated_at = now() where id = $1`,
      [a.id, a.append],
    )
  }

  // ---- mirror PROD -> DEV (Forge runs with APP_ENV=development) -------------
  const src = await prod.query(`select * from storyboard_story where id like $1 order by id`, [STORY_IDS])
  const inDev = await dev.query(`select count(*)::int n from storyboard_story where id like $1`, [STORY_IDS])
  console.log(`[mirror] PROD rows=${src.rowCount} -> DEV (DEV currently holds ${inDev.rows[0].n})`)
  if (APPLY) {
    const cols = src.fields.map((f) => f.name)
    const writable = cols.filter((c) => c !== 'created_at' && c !== 'updated_at')
    for (const row of src.rows) {
      const values = writable.map((c) => row[c])
      const placeholders = writable.map((_, i) => `$${i + 1}`).join(', ')
      const updates = writable.filter((c) => c !== 'id').map((c) => `${c} = excluded.${c}`).join(', ')
      await dev.query(
        `insert into storyboard_story (${writable.join(', ')})
         values (${placeholders})
         on conflict (id) do update set ${updates}, updated_at = now()`,
        values,
      )
    }
    const after = await dev.query(`select count(*)::int n from storyboard_story where id like $1`, [STORY_IDS])
    console.log(`[mirror] DEV now holds ${after.rows[0].n} PROJECTS-WORKSPACE stories`)
  }

  await prod.end()
  await dev.end()
  console.log(APPLY ? 'APPLIED' : 'DRY RUN — re-run with --apply to write')
}

main().catch((err) => { console.error(err); process.exit(1) })
