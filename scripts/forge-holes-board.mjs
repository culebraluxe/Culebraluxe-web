// ---------------------------------------------------------------------------
// FORGE HOLES — Story Board load.
//
// Source of truth: docs/agent/packets/FORGE-HOLES-WORKORDER.md
//   = Grok Forge review vs HEAD 46ad142 (2026-09-12), engine suite 405 pass / 0 fail.
//   Goal: close measured honesty holes. Do not add a second orchestration brain.
//
// The work order lists NINE stories. EIGHT are created here.
//
// Story 6 (FORGE-RECEIPT-PRODUCE-01, "Real release receipt producer") is NOT
// created as its own row: it is the implementation spec of TECH-DEBT-07
// ("Populate releaseEvidence so deploy receipts can exist"), which is already on
// the board In Progress / Critical. A second row would be two writers on one
// story (AGENTS.md: ask first), so instead the producer spec is appended once to
// TECH-DEBT-07 behind a marker — see TECH_DEBT_07_AMENDMENT below.
//
// PROD only. The Board is production control-plane state and Forge never runs
// against DEV; this script fails closed if the PROD connection is absent.
//
// Idempotent: insert-if-absent keyed on id; the TECH-DEBT-07 amendment appends
// only when its marker is missing. Dry-run by default — pass --apply to write.
//
//   node --env-file=.env.local scripts/forge-holes-board.mjs            # dry run
//   node --env-file=.env.local scripts/forge-holes-board.mjs --apply    # write
// ---------------------------------------------------------------------------

import { Pool } from '@neondatabase/serverless'

const APPLY = process.argv.includes('--apply')
const WO = 'docs/agent/packets/FORGE-HOLES-WORKORDER.md'

const PROD_URL = process.env.DATABASE_URL_PROD
if (!PROD_URL) {
  console.error('FAIL CLOSED: DATABASE_URL_PROD is not set. The Board is PROD-only.')
  process.exit(1)
}

// All nine stories belong to one bounded slice: batch 6 (batches 1-5 are taken).
const BATCH = 6
// Points -> priority ladder (lib/story-priority.ts). 8 -> High, 5 -> Medium-High, 3 -> Medium.
const COMMON = {
  workstream: 'TECH',
  operating_surface: 'TECH',
  status: 'Planned',
  completion: 0,
  rollup: true,
  test_mode: 'SCOPED',
  batch: BATCH,
  context_refs: WO,
}

const provenance = (position) =>
  `Work order: FORGE HOLES (Grok review vs HEAD 46ad142, 2026-09-12). Position ${position} of 9 — ` +
  `the order is deliberate; see ${WO}. Authority rules and the never-do list in that file apply ` +
  `to this story. Completion = the acceptance boxes below, not the existence of a commit.`

const STORIES = [
  {
    id: 'FORGE-OBS-SERIAL-01',
    title: 'Observer on serial Smith and Lead',
    priority: 'High',
    points: 8,
    goal:
      'The serial path that actually runs records durable observer events: SCOPE_CHECK, GIT_COMMIT, HOLD and run.end land in workflow_execution_trace_event under source_system=forge_observer, so split child health is no longer the only measured execution path.',
    scope:
      'workflow_app/forge/agent-runtime-role-runner.ts, plus any serial Smith admit / Lead complete path that already computes scopeViolations or HOLD.\n' +
      '- Reuse workflow_app/forge/forge-observer/* and forge-alerts/*.\n' +
      '- Do NOT change HOLD policy.\n' +
      '- Do NOT hook QA/Assay beyond recording run.end / an existing HOLD when that call site is already there and cheap.',
    acceptance_criteria:
      'A serial Smith candidate that leaves allowedScope records SCOPE_CHECK verdict=deny in workflow_execution_trace_event with source_system=forge_observer AND the runner still HOLDs by the existing throw, not by Alerts.\n' +
      'A serial Smith candidate inside scope records SCOPE_CHECK allow + GIT_COMMIT when a commit exists.\n' +
      'A Lead PRE HOLD records HOLD with reason text.\n' +
      'Split path still records: no double-seq collision per attempt; sourceEventId stays storyId:taskId:nodeId:attempt:seq.\n' +
      'pnpm test:observer green; new tests cover the serial hook with a fake sink (no live OpenCode).\n' +
      'pnpm test:forge:engine 0 fail.\n' +
      'Scorecard observerEvents is non-empty after one serial fixture/test write, or after a documented script that inserts via the sink.',
    dependencies:
      'No hard predecessor. The persistent sink and alert rules already exist; the only runner hook today is the split-child block, so this story extends an existing seam rather than inventing one.',
    assay_commands:
      'pnpm test:observer && pnpm test:forge:engine && pnpm exec tsc --noEmit && git diff --check',
    notes: provenance(1),
  },
  {
    id: 'FORGE-SYNC-GUARD-01',
    title: 'Engine-start PROD fail-closed',
    priority: 'High',
    points: 8,
    goal:
      'A Forge lane cannot start when the resolved execution target is not PROD: the guard throws before task claim, before OpenCode spawn and before worktree provision.',
    scope:
      'The start of Forge execution — role runner / invoker / engine-start wrapper, wherever a lane is launched.\n' +
      '- Reuse assertForgeExecutionTarget if it exists; if it only lives on the sync script, lift it to a shared module under workflow_app/forge/.\n' +
      '- Not: rewriting history, not re-running DEV work.',
    acceptance_criteria:
      'With target DEV, Forge lane start throws and does not claim a task or spawn OpenCode.\n' +
      'With target PROD, start proceeds (mocked harness).\n' +
      'Board-sync already-PROD guard still passes.\n' +
      'ENG-FORGE-SYNC-01 part 3 may be marked complete only after this ships; do not mark it complete on a docs commit.\n' +
      'pnpm test:forge:engine 0 fail.',
    dependencies:
      'Closes ENG-FORGE-SYNC-01 part 3 (board: Planned / Critical, "Ship-time board sync"). ENG-FORGE-SYNC-01 stays open until this ships.',
    assay_commands: 'pnpm test:forge:engine && pnpm exec tsc --noEmit && git diff --check',
    notes:
      provenance(2) +
      '\n\nContext: MEMORY says Forge never runs against DEV and board-sync refuses non-PROD, but engine / role-runner start could still launch a lane when APP_ENV resolved to DEV. That is how the board went dark (WS series in DEV, board in PROD).',
  },
  {
    id: 'FORGE-SESSION-ID-01',
    title: 'Persist OpenCode session id per attempt',
    priority: 'Medium-High',
    points: 5,
    goal:
      'Each attempt stores the OpenCode session id it actually used, so per-attempt cost and raw tokens can later be attributed to a real session instead of guessed.',
    scope:
      'agent-runtime/opencode/opencode-client.ts\n' +
      '- agent-runtime/opencode/opencode-harness-adapter.ts\n' +
      '- Evidence / work-item / run columns that already exist for session or metadata: prefer an existing JSON/notes/metadata field over a new migration if one can hold opencodeSessionId.\n' +
      '- New column only if nothing honest exists; if added, add it to DEV and PROD together.',
    acceptance_criteria:
      'Successful OpenCode fixture/test persists a non-empty session id when the fake handle provides one.\n' +
      'Failed spawn persists null, not "unknown".\n' +
      'Scorecard still reports tokens/cost NOT CAPTURED (this story does not fill those columns).\n' +
      'No widgets-to-dollars math.\n' +
      'pnpm test:forge:engine 0 fail.',
    dependencies:
      'Unlocks ENG-FORGE-COST-01 (board: Planned / Medium, per-attempt usage capture), which cannot attribute cost without a stored session id. Filling cost_usd via opencode export is parked by the work order and needs this story first.',
    assay_commands: 'pnpm test:forge:engine && pnpm exec tsc --noEmit && git diff --check',
    notes:
      provenance(3) +
      '\n\nTechnical fix: capture the session id OpenCode actually used (stdout/stderr parse, or the API the client already has). Empty means null, never a fake id. Persist it on the attempt evidence as opencodeSessionId. Do not call opencode export in this story — this story only stores the key. The session continuity marker (.forge-session.continue) stays env-gated, default OFF.\n\nIssue: the client may pass --session / --continue but the new session id is not stored on the attempt, and OpenCodeRunResult has no usage field. Do not invent usage.',
  },
  {
    id: 'FORGE-OBS-LIST-01',
    title: 'Alert rules can see prior attempts',
    priority: 'Medium-High',
    points: 5,
    goal:
      'Alert rules can see the previous attempts of the same story and node after a process restart, so RETRY_UNCHANGED_INPUT is truthful instead of memory-only.',
    scope:
      'workflow_app/forge/forge-observer/persistent-sink.ts\n' +
      '- Optional: a listFromTrace reader over workflow_execution_trace_event where source_system=forge_observer.\n' +
      '- Alerts stay pure over TraceEvent[].',
    acceptance_criteria:
      'Test: persist two HOLD events with the same retryHash via the write fake, load(), evaluateAlerts returns RETRY_UNCHANGED_INPUT.\n' +
      'Test: load of an empty story returns [], not throw.\n' +
      'The serial hook from story 1 is still log-only.\n' +
      'pnpm test:observer and pnpm test:forge:engine 0 fail.',
    dependencies:
      'Best done after FORGE-OBS-SERIAL-01, which is what starts writing serial events worth reading back. Not a hard block.',
    assay_commands:
      'pnpm test:observer && pnpm test:forge:engine && pnpm exec tsc --noEmit && git diff --check',
    notes:
      provenance(4) +
      '\n\nIssue: list() is the in-process memory sink and the durable write is fire-and-forget, so RETRY_UNCHANGED_INPUT cannot see attempt N-1 after a process restart.\n\nTechnical fix: add an async load(storyId) (or a sync cache fill at run start) that maps trace rows back to TraceEvent — underscore types to dotted kinds. Runner: before evaluateAlerts on attempt >= 2, load prior events for that storyId + nodeId. Mapping must be loss-tolerant: missing optional fields stay undefined. The write still never throws.',
  },
  {
    id: 'FORGE-PARITY-CHECK-01',
    title: 'Parity compares check constraints',
    priority: 'Medium-High',
    points: 5,
    goal:
      'pnpm db:parity detects CHECK-constraint drift, so a check that exists in PROD and not in DEV can no longer be reported as zero drift.',
    scope:
      'The parity script / comparator (the pnpm db:parity implementation).\n' +
      '- Apply the missing CHECK to DEV only, copying the PROD definition. Do not alter PROD.\n' +
      '- Do not force the two malformed DEV rows into PROD.',
    acceptance_criteria:
      'Before the DEV constraint: parity fails with a named missing check.\n' +
      'After applying the check to DEV: that item disappears from the diff.\n' +
      'PROD constraint text unchanged.\n' +
      'No ON CONFLICT / insert of the two bad rows into PROD.\n' +
      'pnpm test:forge:engine 0 fail (or parity unit tests if they exist).',
    dependencies:
      'Relates to the DEV_OPS playbook release gates (pnpm db:parity, pnpm db:migrations). Never-do rule 5 applies: do not weaken the PROD check to match DEV.',
    assay_commands: 'pnpm db:parity && pnpm test:forge:engine && git diff --check',
    notes:
      provenance(5) +
      '\n\nIssue: PROD has agent_work_item_parallel_shape_check, DEV does not. pnpm db:parity reported 0 drift because it compares tables/columns/indexes/FKs, not CHECKs — so DEV accepted rows PROD rejected. Two malformed parallel-shape rows remain in DEV on purpose.\n\nTechnical fix: extend parity to list check constraints per table (name + src) on both databases and diff them. Report constraint-only drift as drift (non-zero exit). Add the PROD agent_work_item_parallel_shape_check to DEV via a migration that is safe if already present. Document the two leftover DEV rows as out of scope — investigate, do not weaken the check.',
  },
  {
    id: 'FORGE-ARCH-BYPASS-01',
    title: 'Close the 6 cruiser hits (or quarantine with a ticket)',
    priority: 'Medium-High',
    points: 5,
    goal:
      'The dependency-cruiser architecture gate reports zero of the six cited UI-to-db violations, so QA no longer fails closed on architecture for pre-existing imports.',
    scope:
      'Only the 6 cited files (confirm the current list with pnpm forge:tools --run / the gate artifact — do not expand into a rewrite).\n' +
      '- Route reads through existing services or a thin already-used facade.\n' +
      '- Not: redesigning the review UI.',
    acceptance_criteria:
      'pnpm forge:tools --run architecture gate archRan:true and zero of these 6 paths in violations.\n' +
      "No new from 'db/ or from '../db/ imports in those files.\n" +
      'pnpm exec tsc --noEmit clean for the touched files.\n' +
      'Review pages still typecheck; no behavior change is required beyond the import path.',
    dependencies:
      'ENG-FORGE-V5-25 (dependency-cruiser Architecture Invariant Gate, board: In Progress) is what surfaced these 6 violations. Companion, not a hard block.',
    assay_commands: 'pnpm forge:tools --run && pnpm exec tsc --noEmit && git diff --check',
    notes:
      provenance(7) +
      '\n\nIssue: the first live dependency-cruiser run found 6 violations — review-storyboard, review-dashboard (x3) and form-editor-surface (x2) importing db/ directly. The architecture hard gate is now real, so leaving them means QA fails closed on arch.\n\nTechnical fix: replace the db/ imports with the domain service already used elsewhere for that read, or one shared server facade if one exists. If a call is inherently admin/debug and has no service, add a NAMED cruiser exception carrying a story id in the comment — max 2 exceptions, each with a follow-on story id. Prefer zero exceptions.',
  },
  {
    id: 'FORGE-PACKET-OBS-01',
    title: 'Context packet compiler (observer mode)',
    priority: 'High',
    points: 8,
    goal:
      'A provenance-bearing context packet — seams, assignment files, 1-hop dependencies, assay commands and the AGENTS excerpt, plus a manifest — is compiled and persisted in observer mode, without replacing the live prompt.',
    scope:
      'New module workflow_app/forge/forge-context-packet.ts (pure compile + types).\n' +
      '- Inputs: story id, node, attempt, Scout findings text, Architect seams, accepted Lead assignment paths, frozen assay commands, and the AGENTS excerpts already used in prompts.\n' +
      '- Output manifest: { sources: [{ path or ref, reason, bytes, hash? }], omittedCount, estimatedTokens }.\n' +
      '- Observer mode: compile and persist on evidence / observer event. Do NOT replace the live prompt in this story.\n' +
      '- Split child: the packet must not include sibling implementation files; collision/integration facts only if already on the assignment.',
    acceptance_criteria:
      'Unit: same seams + assignment produce an identical manifest source list.\n' +
      "Unit: a sibling file of a split child is omitted unless it is in that child's allowedScope.\n" +
      'Unit: incomplete required context sets incomplete: true and does not throw.\n' +
      'No change to FORGE_SDLC transitions.\n' +
      'pnpm test:forge:engine 0 fail.',
    dependencies:
      'Feature (Maestro acquisition #1), observer-mode only. Highest-value new work after holes 1-3; it changes what OpenCode would receive, not who routes.',
    assay_commands: 'pnpm test:forge:engine && pnpm exec tsc --noEmit && git diff --check',
    notes:
      provenance(8) +
      '\n\nIssue: Scout has Ripwire --pack-task and Lead/Smith get large prompts, but there is no provenance-bearing packet with a manifest.\n\nTechnical fix: selection takes required sources first, then expands assignment files to DIRECT import neighbors only when the import specifier is relative (same idea as the Maestro indexer, bounded depth 1). Skip node_modules. Missing required context sets manifest flag incomplete: true plus a reason — do not scan the whole repo. Deterministic: same inputs produce the same source list (sort paths). Record detail.packetHash on run.start once the serial observer hook exists.',
  },
  {
    id: 'FORGE-SCORECARD-BOARD-01',
    title: 'Publish scorecard as a read-only CLI + note',
    priority: 'Medium',
    points: 3,
    goal:
      'pnpm forge:scorecard is the operator contract for Forge honesty: it prints outcomes, roles, splitHealth and telemetry.note verbatim, exits 0 even when rates are null, and refuses to write.',
    scope:
      'The scripts/ CLI, if not already complete.\n' +
      '- Do not add a dashboard page unless one already exists to hang a panel on.\n' +
      '- Read-only PROD.',
    acceptance_criteria:
      'pnpm forge:scorecard 30 runs against the PROD read path (or documented execute injection) and prints NOT CAPTURED for tokens/cost while those columns are empty.\n' +
      'Does not insert rows.\n' +
      'Documented in docs/agent/MEMORY.md in one line: command + honesty rules.',
    dependencies:
      'Feature, no hard predecessor. The scorecard function already exists (pnpm forge:scorecard); this story makes the command the operator contract and pins the honesty rules so a model cannot "fix" nulls to zero.',
    assay_commands: 'pnpm forge:scorecard 30 && git diff --check',
    notes: provenance(9),
  },
]

// ---------------------------------------------------------------------------
// Work-order story 6 (FORGE-RECEIPT-PRODUCE-01, "Real release receipt producer")
// is deliberately NOT a board row here. It is the implementation spec of
// TECH-DEBT-07 — already on the board In Progress / Critical, "Populate
// releaseEvidence so deploy receipts can exist". Two rows would be two writers
// on one story, so the producer spec is folded into TECH-DEBT-07 once, behind
// this marker. The work order's own guard is preserved verbatim.
// ---------------------------------------------------------------------------
const DEBT07_ID = 'TECH-DEBT-07'
const DEBT07_MARK = 'AMENDED 2026-09-12 — FORGE HOLES work-order story 6'
const DEBT07_AMENDMENT =
  `\n\n${DEBT07_MARK}: the producer half is FORGE-RECEIPT-PRODUCE-01 in the FORGE HOLES work order ` +
  `(${WO}), deliberately not a separate board row — this story is where that work lands.\n` +
  '- Issue: forge-release-receipt.ts rejects placeholders, but nothing produces releaseEvidence from a real host signal, so deploy/production_smoke still blocks or waits and MISSING_DEPLOY_RECEIPT stays watch forever.\n' +
  '- Scope: workflow_app/forge/forge-release-receipt.ts (keep the fail-closed assessor); the DevOps / publish path that already talks to Vercel or the host actually used; AgentRunEvidence.releaseEvidence. Not: fabricating receipts for WS-14, not marking WS-14 Complete.\n' +
  '- Technical fix: releaseReceiptFromDeploymentSignal already returns null without a signal — wire the ACTUAL deploy adapter output (deployment id + artifact sha + url or equivalent) into that function. If a run is a recorded batch deferral, persist a TYPED deferral (isRecordedDeploymentDeferral), not a fake receipt. The gate continues to reject n/a, tbd, waived and non-sha artifacts.\n' +
  '- Acceptance: a fixture provider payload with id + sha yields evidence containing a receipt assessReleaseReceipt accepts; no provider payload means the producer returns null and the gate does not pass on a placeholder; an explicit deferral flag yields a deferral not a receipt, so scorecard/alerts see watch rather than a fake pass; the 10 existing receipt tests still pass plus new producer tests; do not flip TECH-DEBT-07 / WS-14 to Complete unless a real PROD deploy has been observed — if none, leave it In Progress and say so.\n' +
  '- Assay: pnpm test:forge:engine && pnpm exec tsc --noEmit && git diff --check'

async function main() {
  const pool = new Pool({ connectionString: PROD_URL })
  console.log(`target host: ${new URL(PROD_URL).host}`)
  console.log(`${APPLY ? 'APPLY' : 'DRY RUN (pass --apply to write)'} — ${STORIES.length} stories, batch ${BATCH}`)

  const ids = [...STORIES.map((s) => s.id), DEBT07_ID]
  const existing = await pool.query(
    'select id, status from storyboard_story where id = any($1::text[])',
    [ids],
  )
  const byId = new Map(existing.rows.map((r) => [r.id, r]))

  let created = 0
  let skipped = 0
  for (const s of STORIES) {
    const found = byId.get(s.id)
    if (found) {
      console.log(`SKIP   ${s.id} — already on the board (${found.status})`)
      skipped += 1
      continue
    }
    console.log(`CREATE ${s.id} — ${s.title} [${s.priority}, ${s.points} pts]`)
    created += 1
    if (!APPLY) continue
    await pool.query(
      `insert into storyboard_story (
         id, workstream, title, priority, status, notes, batch, goal, scope,
         acceptance_criteria, dependencies, completion, rollup, test_mode,
         operating_surface, context_refs
       ) values ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13,$14,$15,$16)`,
      [
        s.id, COMMON.workstream, s.title, s.priority, COMMON.status, s.notes, COMMON.batch,
        s.goal, s.scope, s.acceptance_criteria, s.dependencies ?? null, COMMON.completion,
        COMMON.rollup, COMMON.test_mode, COMMON.operating_surface, COMMON.context_refs,
      ],
    )
  }

  const debt = await pool.query('select notes from storyboard_story where id = $1', [DEBT07_ID])
  if (debt.rowCount === 0) {
    console.log(`\nWARN   ${DEBT07_ID} is not on the board — work-order story 6 could not be folded in.`)
  } else if (debt.rows[0].notes.includes(DEBT07_MARK)) {
    console.log(`\nSKIP   ${DEBT07_ID} amendment — marker already present`)
  } else {
    console.log(`\nAMEND  ${DEBT07_ID} — append the story 6 producer spec`)
    if (APPLY) {
      await pool.query(
        'update storyboard_story set notes = notes || $2, updated_at = now() where id = $1',
        [DEBT07_ID, DEBT07_AMENDMENT],
      )
    }
  }

  console.log(`\n${APPLY ? 'APPLIED' : 'DRY RUN'}: ${created} create, ${skipped} skip`)

  if (APPLY) {
    const after = await pool.query(
      'select id, status, priority, batch, completion from storyboard_story where id = any($1::text[]) order by id',
      [ids],
    )
    console.log('\nboard state after:')
    for (const r of after.rows) {
      console.log(`  ${r.id.padEnd(26)} ${r.status.padEnd(10)} ${String(r.priority).padEnd(12)} batch ${r.batch}  ${r.completion}%`)
    }
  }
  await pool.end()
}

main().catch((e) => {
  console.error('FAILED:', e.message)
  process.exit(1)
})
