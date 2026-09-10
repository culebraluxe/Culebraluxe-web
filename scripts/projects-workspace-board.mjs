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
// --context-only writes ONLY the captain-context scope notes + context_refs (to PROD
// and DEV directly) and exits. Needed because the PROD->DEV mirror below copies EVERY
// column including `status`, so a full --apply would clobber DEV's live run state
// (Complete / In Progress) with PROD's Planned.
const CONTEXT_ONLY = process.argv.includes('--context-only')
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

// ---------------------------------------------------------------------------
// Phase 2 — make the assay MACHINE-RUNNABLE.
//
// Review found GPT's assay_commands are PROSE, not commands (WS-07 has no command
// at all; WS-13 requires screenshots / a manual keyboard pass). Forge freezes
// assay_commands into the legal proof vocabulary and then the assay harness must RUN
// them, so prose cannot be satisfied by a chunk and human steps cannot be automated.
// This repo has node:test + tsx only (no Playwright/vitest/jsdom), so UI/visual
// acceptance is not machine-verifiable.
//
// Each story therefore gets:
//   * assay_commands  = concrete, scoped, runnable commands only;
//   * notes += FORGE MACHINE ASSAY + HUMAN GATE (the visual/manual items, listed so
//     nobody pretends Forge verified them).
// The original prose is preserved verbatim in notes. Phase 2 also adds WS-18, the
// single human visual/accessibility gate that collects every manual item.
// ---------------------------------------------------------------------------

const MACHINE_ALWAYS = ['- `pnpm exec tsc --noEmit`', '- `git diff --check`']

// id -> { test: story-owned test file Forge must create (red -> green), human: manual items }
const ASSAY_MAP = {
  'PROJECTS-WORKSPACE-01': { test: 'testv2/projects-workspace-01-readmodel.test.ts', human: ['pane content read-through against the approved reference'] },
  'PROJECTS-WORKSPACE-02': { test: 'testv2/projects-workspace-02-tokens.test.ts', human: ['desktop geometry match', 'approved browser screenshots (three-pane widths, borders, sticky behaviour)'] },
  'PROJECTS-WORKSPACE-03': { test: 'testv2/projects-workspace-03-navigator.test.ts', human: ['tree interaction feel', 'icon-language review'] },
  'PROJECTS-WORKSPACE-04': { test: 'testv2/projects-workspace-04-header-nextaction.test.ts', human: ['header/callout visual match'] },
  'PROJECTS-WORKSPACE-05': { test: 'testv2/projects-workspace-05-workplan-table.test.ts', human: ['table alignment/overflow visual check'] },
  'PROJECTS-WORKSPACE-06': { test: 'testv2/projects-workspace-06-inspector.test.ts', human: ['edit feedback feel', 'stale-edit UX review'] },
  'PROJECTS-WORKSPACE-07': { test: 'testv2/projects-workspace-07-actions.test.ts', human: ['linked-record navigation spot check (Forms/signature/Cabinet/Marketing/Accounting)'] },
  'PROJECTS-WORKSPACE-08': { test: 'testv2/projects-workspace-08-timeline.test.ts', human: ['timeline grouping read-through'] },
  'PROJECTS-WORKSPACE-09': { test: 'testv2/projects-workspace-09-calendar.test.ts', human: ['calendar projection read-through'] },
  'PROJECTS-WORKSPACE-10': { test: 'testv2/projects-workspace-10-documents.test.ts', human: ['documents lens read-through'] },
  'PROJECTS-WORKSPACE-11': { test: 'testv2/projects-workspace-11-activity.test.ts', human: ['activity feed read-through'] },
  'PROJECTS-WORKSPACE-12': { test: 'testv2/projects-workspace-12-urlstate.test.ts', human: ['back/forward and breadcrumb walkthrough'] },
  'PROJECTS-WORKSPACE-13': { test: 'testv2/projects-workspace-13-semantics.test.ts', human: ['iPad verification', 'accessibility scan', 'manual keyboard pass', 'state screenshots'] },
  'PROJECTS-WORKSPACE-14': { test: 'testv2/projects-workspace-14-acceptance.test.ts', human: ['DEV/PROD smoke on the canonical fixture'] },
  'PROJECTS-WORKSPACE-15': { test: null, extra: ['- `node --env-file=.env.local --import tsx scripts/probe-error-capture.ts`'], human: [] },
  'PROJECTS-WORKSPACE-16': { test: 'testv2/projects-service-projection.test.ts', human: [] },
  'PROJECTS-WORKSPACE-17': { test: 'testv2/projects-workspace-17-scale.test.ts', human: [] },
}

const GATE_STORY = {
  id: 'PROJECTS-WORKSPACE-18',
  title: 'Human visual, iPad, and accessibility acceptance gate',
  priority: 'High',
  batch: 4,
  goal:
    'Collect every visual, touch, keyboard and accessibility acceptance item that cannot be machine-verified into one explicit human gate, so no automated run can claim them.',
  scope:
    'The approved three-pane reference, iPad/touch behavior, keyboard traversal and screen-reader semantics across the workspace panes, plus the state screenshots.',
  acceptance_criteria: [
    'Every item listed as HUMAN GATE on stories 02-14 is walked on the real screen and recorded with a screenshot or a written pass/na note.',
    'Desktop, medium and small pane behavior is confirmed against the approved reference.',
    'Practical touch targets (~48px) and focus transfer to drawers are confirmed on iPad.',
    'Keyboard traversal and screen-reader semantics are confirmed for tree, tabs, table and forms.',
    'This gate is EXPLICITLY human: no automated run may mark it complete, and Forge must not be asked to verify it.',
  ].join('\n'),
  dependencies: 'PROJECTS-WORKSPACE-02, PROJECTS-WORKSPACE-03, PROJECTS-WORKSPACE-04, PROJECTS-WORKSPACE-05, PROJECTS-WORKSPACE-06, PROJECTS-WORKSPACE-13',
  notes:
    'BASELINE STATUS: MISSING as a story. Shows up as scattered manual steps inside stories 02/05/13, which Forge cannot execute (this repo has node:test + tsx only; no browser test tooling). ADDED 2026-09-10 to keep machine acceptance and human acceptance honestly separate.',
  test_mode: 'SCOPED',
  assay_commands: '- HUMAN GATE — not machine-verifiable by design; see the HUMAN GATE items on stories 02-14.',
}

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

// ---- captain context (2026-09-10) ------------------------------------------
// Two stories were SILENT about shipped code, and the WS-09 architect duly began
// designing a calendar from scratch. These notes point the Scout packet
// (context_refs) and the Architect at what already exists. They add NO new scope:
// the calendar is reused, and the document vault is a lens, not a new store.
const MARK_CTX = 'AMENDED 2026-09-10 (captain context)'

const CAPTAIN_CONTEXT = [
  {
    id: 'PROJECTS-WORKSPACE-09',
    append:
      `\n\n${MARK_CTX}: REUSE the shipped Catch-up calendar engine — ` +
      'components/portal/catch-up-calendar.tsx (month/week toggle, initialView="month"), ' +
      'lib/catchup/calendar-mappers.ts (event mappers), components/portal/fullcalendar-candidate.tsx ' +
      '(daygrid + timegrid). Default MONTH with WEEK available, exactly as Catch-up already does; ' +
      'do not build a second calendar engine. ' +
      'KEY DATES are DERIVED from canonical dated facts — never a new date store and never hand-entered: ' +
      'wbs_item.due_at (WBS commitments), project.starts_at / project.ends_at (project lifecycle), and ' +
      'canonically anchored appointments reachable through the existing calendar intake ' +
      '(calendar_intake_receipt / google_calendar_token_store). WBS dates are DEADLINES; ' +
      'intake-linked items are APPOINTMENTS — keep the two visually and semantically distinct, and never ' +
      'invent an event for a row that has no date. ' +
      'The pane must NOT read Apple/provider types: the Catch-up pane treats Apple Calendar as authoritative ' +
      'for ITS OWN feed, but the project calendar projects canonical DB facts (project -> wbs_item -> dates). ' +
      'If an expected date source is absent, say so on screen; do not silently drop the event.',
  },
  {
    id: 'PROJECTS-WORKSPACE-10',
    append:
      `\n\n${MARK_CTX}: the vault ALREADY EXISTS as transaction_document (+ document_form_instance). ` +
      'This story is a project LENS over it, NOT a new document store. Association is schema-driven, never ' +
      'guessed: PROJECT via transaction_document.deal_id (and contract_id where present); PERSON via ' +
      'transaction_document.party_person_id and document_form_instance.person_id; PROPERTY is NOT a direct ' +
      'column on transaction_document — reach it through the deal, or via ' +
      'document_form_instance.property_id, and explain the no-property case instead of inventing one. ' +
      'VERSIONS/history via supersedes_document_id + issued_version; FILES via media_id / signed_media_id ' +
      '(media, property_media.role=document). Render type/state/issued date/context and group current vs ' +
      'superseded versions.',
  },
]

const CONTEXT_REFS = {
  'PROJECTS-WORKSPACE-09': [
    'components/portal/catch-up-calendar.tsx',
    'lib/catchup/calendar-mappers.ts',
    'components/portal/fullcalendar-candidate.tsx',
    'components/portal/catch-up-calendar-evaluation.tsx',
    'app/portal/catch-up/page.tsx',
    'ui/projects/source.ts',
    'ui/projects/work-plan-projection.ts',
    'components/portal/projects-workspace.tsx',
    'schema: wbs_item.due_at, project.starts_at, project.ends_at',
    'schema: calendar_intake_receipt, google_calendar_token_store',
  ].join('\n'),
  'PROJECTS-WORKSPACE-10': [
    'app/portal/documents/page.tsx',
    'app/portal/documents/[documentId]/page.tsx',
    'components/portal/documents/document-list.tsx',
    'ui/projects/source.ts',
    'schema: transaction_document (deal_id, contract_id, party_person_id, media_id, signed_media_id, supersedes_document_id, issued_version, state, document_type)',
    'schema: document_form_instance (deal_id, person_id, property_id, contract_id, showing_id)',
    'schema: media, property_media.role=document',
  ].join('\n'),
}

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

  // ---- captain context: scope notes + Scout packet refs ---------------------
  // Written to PROD and DEV directly so DEV does not depend on the status-clobbering
  // mirror below. --context-only writes these and exits.
  const writeContext = APPLY
  for (const a of CAPTAIN_CONTEXT) {
    for (const [label, conn] of [['PROD', prod], ['DEV', dev]]) {
      const cur = await conn.query(`select scope as v from storyboard_story where id = $1`, [a.id])
      const existing = cur.rows[0]?.v ?? ''
      if (existing.includes(MARK_CTX)) {
        console.log(`[context] ${label} ${a.id} already carries the captain context — skipped`)
        continue
      }
      console.log(`[context] ${label} ${a.id}.scope += captain context`)
      if (!writeContext) continue
      await conn.query(
        `update storyboard_story set scope = coalesce(scope,'') || $2, updated_at = now() where id = $1`,
        [a.id, a.append],
      )
    }
  }
  for (const [id, refs] of Object.entries(CONTEXT_REFS)) {
    console.log(`[context] ${id}.context_refs = ${refs.split('\n').length} ref(s)`)
    if (!writeContext) continue
    for (const conn of [prod, dev]) {
      await conn.query(`update storyboard_story set context_refs = $2, updated_at = now() where id = $1`, [id, refs])
    }
  }
  if (CONTEXT_ONLY) {
    await prod.end()
    await dev.end()
    console.log(writeContext ? 'CONTEXT-ONLY APPLIED (PROD + DEV)' : 'CONTEXT-ONLY DRY RUN')
    return
  }

  // ---- phase 2: machine-runnable assays + the explicit human gate -----------
  const prodAll = await prod.query(`select id, assay_commands, notes from storyboard_story where id like $1 order by id`, [STORY_IDS])
  for (const row of prodAll.rows) {
    const spec = ASSAY_MAP[row.id]
    if (!spec) continue
    const commands = [
      ...MACHINE_ALWAYS,
      ...(spec.test ? [`- \`pnpm exec tsx --test ${spec.test}\``] : []),
      ...(spec.extra ?? []),
    ].join('\n')
    const gate = spec.human.length
      ? `\n\nHUMAN GATE (NOT machine-verifiable — this repo has node:test + tsx only; see PROJECTS-WORKSPACE-18):\n${spec.human.map((h) => `  * ${h}`).join('\n')}`
      : '\n\nHUMAN GATE: none — fully machine-verifiable.'
    const notes = String(row.notes ?? '')
    const marker = 'FORGE MACHINE ASSAY'
    console.log(`[assay] ${row.id} -> ${spec.test ?? 'commands only'}${spec.human.length ? ' + human gate' : ''}`)
    if (!APPLY) continue
    const withoutOldProse = notes.includes(marker) ? notes.split(`\n\nORIGINAL ASSAY PROSE`)[0] : notes
    const preserved = notes.includes('ORIGINAL ASSAY PROSE')
      ? ''
      : `\n\nORIGINAL ASSAY PROSE (advisory, preserved for intent — NOT a machine proof):\n${String(row.assay_commands ?? '').replace(/\n/g, '\n  ')}`
    await prod.query(
      `update storyboard_story set assay_commands = $2, notes = $3, updated_at = now() where id = $1`,
      [row.id, commands, `${withoutOldProse}${preserved}\n\n${marker}:\n${commands}${gate}`],
    )
  }

  // ---- add the single human gate story -------------------------------------
  console.log(`[add] ${GATE_STORY.id} — ${GATE_STORY.title}`)
  if (APPLY && workstream && surface) {
    await prod.query(
      `insert into storyboard_story
         (id, workstream, title, priority, status, notes, batch, goal, scope, acceptance_criteria,
          dependencies, operating_surface, test_mode, assay_commands, completion, rollup)
       values ($1,$2,$3,$4,'Planned',$5,$6,$7,$8,$9,$10,$11,$12,$13,0,false)
       on conflict (id) do update set
         title = excluded.title, priority = excluded.priority, batch = excluded.batch, goal = excluded.goal,
         scope = excluded.scope, acceptance_criteria = excluded.acceptance_criteria,
         dependencies = excluded.dependencies, notes = excluded.notes,
         operating_surface = excluded.operating_surface, test_mode = excluded.test_mode,
         assay_commands = excluded.assay_commands, updated_at = now()`,
      [GATE_STORY.id, workstream, GATE_STORY.title, GATE_STORY.priority, GATE_STORY.notes, GATE_STORY.batch,
        GATE_STORY.goal, GATE_STORY.scope, GATE_STORY.acceptance_criteria, GATE_STORY.dependencies,
        surface, GATE_STORY.test_mode, GATE_STORY.assay_commands],
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
