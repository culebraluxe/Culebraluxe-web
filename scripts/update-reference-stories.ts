// ONE-OFF: bring the two durable reference stories back in line with the live system.
//
//   node --env-file=.env.local --import tsx scripts/update-reference-stories.ts
//
// ARCH-HANDOFF was last amended 2026-08-25, i.e. BEFORE the service tier and the MVI page
// runtime existed (they landed 2026-09-06..09-11). SOP1 was written 2026-08-22, before Forge
// was pinned to PROD-only, before the schema-parity and migration-ledger release gates, and
// before the analyzer-tool capability boundary. A future session reading either one as
// authority describes a system that has moved.
//
// This script is idempotent: each amendment is guarded by a marker string, so a second run
// reports "already present" and writes nothing. It also repairs a real defect in the stored
// ARCH-HANDOFF brief: two blocks were stored with LITERAL backslash-n instead of newlines,
// which made the read-this-first invariants block render as one unreadable line.
//
// NOTE: this deliberately uses an explicit PROD pool rather than the application gateway.
// db/database-gateway.ts binds its executor at MODULE LOAD, so a script that flips APP_ENV in
// its own body still connects to whatever environment was set when the gateway was imported —
// while dbTargetInfo() cheerfully reports the new target. That trap is DEEP1 section 6, and it
// has already written to the wrong branch once.
import { Pool } from '@neondatabase/serverless'

import { createPoolExecutor } from './lib/pool-executor'
import { getStoryboardStory, updateStoryboardStory } from '../db/storyboard'

const MVI_MARKER = 'THE SERVICE TIER AND THE MVI SCREEN RUNTIME'
const SOP_MARKER = 'OPERATOR AMENDMENT 2026-09-11/12'
const INVARIANT_ANCHOR = '- Persist important learning so each failure makes the factory better.'

const NEW_INVARIANTS = `${INVARIANT_ANCHOR}
- The application has a typed SERVICE TIER (services/) and an MVI page runtime (ui/). A screen
  reaches the database through a controller -> service -> repository, never directly.
- The SCREEN is the contract. Read the screen before inventing a mapping, and keep exactly ONE
  implementation of any vocabulary it declares.
- A schema story is not delivered until DEV and PROD both match the released code; see the
  Database Delivery Rule.

THREE REFERENCE STORIES, READ IN THIS ORDER
1. ARCH-HANDOFF (this row) - architecture, boundaries, operating model.
2. DEEP1 - the data pipeline (ODS -> warehouse -> screen) and the traps that cost real hours.
3. SOP1 - factory/queue health doctrine for whoever is watching Forge.`

const ARCH_AMENDMENT = `

================================================================================
${MVI_MARKER} - SETTLED ARCHITECTURE (AMENDMENT 2026-09-11/12)
================================================================================

WHY THIS SECTION EXISTS
Everything above this line was written when portal screens read the database through ad-hoc
loaders. That is no longer the architecture. As of 2026-09-11 there is a typed service kernel
(services/), an MVI page runtime (ui/), a glass-box test tier (testv2/), and four portal screens
wired through them end to end. An architect who reads only the sections above will describe an
application that has moved. Read this section as the current shape, and treat the older sections
as still-true doctrine about the factory, the control plane and the operating model.

1. THE SERVICE TIER (services/)
services/ is a typed service kernel with 14 domains: comms, contract, core, entitlement, firm,
forms, person, project, property, regrid, security, showing, vault, wbs.

The seam is the composition root, and it is the ONLY one:
  services/composition.ts -> composeCoreServices(repositories, infrastructure)
It builds the kernel once, registers every domain in a ServiceRegistry, injects infrastructure,
and returns typed handles (CoreServiceComposition): registry, person, firm, property, contract,
showing, security, wbs, project - plus comms, form and vault, which are OPTIONAL and present only
when the composition was actually given their repository (the comment calls that "the full
runtime"). Do not build a second composition root, and do not construct a service by hand. If a
domain is missing at runtime, it is missing because its repository was not passed in.

services/index.ts NO LONGER EXISTS. It was removed as a dead barrel; the public seam is the
composition root plus each domain's own index.ts. services/core/index.ts is the most depended-on
file in the tier (30 dependents across services/ and ui/), which is what makes the kernel
contracts stable.

Kernel contracts live in services/core/:
- envelopes and results: ServiceEnvelope, ServiceResult (ServiceSuccess | ServiceFailure),
  ServiceErrorShape, ServiceFailure.
- operation shapes: ServiceOperationContract, ServiceOperationDefinition(s), ServiceOperationMap,
  ServiceOperationName, ServiceOperationKind, ServiceExecutionMode, ServiceExecutionPolicy.
- identity and authority: ServicePrincipal, ServiceActor, ServiceResourceContext,
  ServiceCapability, ServiceEndpoint, ServiceDescriptor.
- ServiceInfrastructure: the injected ports.

FOUR PORTS ARE EFFECTIVELY MANDATORY, and the composition enforces it:
- audit (defaultAuditPort), events (defaultEventPort), errors (defaultErrorSink - the
  ServiceErrorSink seam), and authorization.
- Authorization defaults to an ENFORCED resolver, and the reason is written in the code: "a
  kernel is never built without an authorization decision source." Do not construct a kernel that
  skips these ports. That is exactly how an unaudited mutation gets shipped.

Failure semantics inside the tier (the same split as the repo-wide error-capture obligation):
- EXPECTED business outcomes - validation failures, FORBIDDEN/authorization denials, not-found -
  are domain failures returned in the envelope. They are audited control flow, NOT error rows.
- Anything UNHANDLED is captured through the errors sink with domain, operation and
  correlationId. Never a bare try/catch, never console.error only.

Repository boundary rule (already true in this tier): repository boundaries own normalization of
driver-native values into stable application contracts. Nothing above the repository should ever
see a Date, BigInt, Buffer or driver-specific object from Neon/Postgres.

A component importing from db/* is a defect, and it is now machine-checked: commit f1bc0a3 fixed
the 6 component -> db violations that the new architecture gate found.

2. THE MVI SCREEN RUNTIME (ui/)
ui/runtime/ is a transport- and framework-neutral Model-View-Intent page runtime. Its reason to
exist is stated in the file itself: dispatch ingress, model publication and concurrency semantics
"otherwise get reimplemented in React components."

- BasePageController<TModel, TMap> implements PageStore<TModel>. A concrete controller declares
  exactly one thing the runtime cares about: operations: PageOperationDefinitions<TModel, TMap>.
- The parent owns dispatch, publication and concurrency; the controller owns what each intent
  does. The protected helpers replaceModel/updateModel are how a controller publishes, and
  dispose() aborts every in-flight operation and stops publication.

THREE EXECUTION MODES, declared per operation (PageOperationDefinition.execution):
- parallel (the default) - the operation gets its own AbortController and an isCurrent() gate
  before any model write.
- latest - LATEST-WINS. Dispatching the same operation again ABORTS the previous in-flight one.
  This is the search-as-you-type / filter / paging case; do not hand-roll it in a component.
- serial - per-operation chaining (serialTails), so an operation can never interleave with itself
  even when dispatched twice in a row.

Context handed to a handler (PageOperationContext): signal, snapshot(), update(reducer),
isCurrent(). An operation that writes the model after it has been superseded is exactly the bug
class the runtime exists to prevent. Guard writes with context.update(), which already checks
disposed/aborted/isCurrent, instead of setting state directly.

React NEVER drives state. The React-facing contract is PageStore: snapshot() + subscribe(), bound
through ui/runtime/use-page-controller.ts. Components render the published model and dispatch
intents; nothing else.

Projections are PURE and separate from both the source and the controller:
ui/projects/service-projection.ts (mapRealProjectsToWorkspace), ui/projects/tree-projection.ts,
ui/client-workspace/channel-projection.ts. A projection maps service DTOs into a page model. It
does not fetch, and it does not import React.

Every screen has a SOURCE interface with a real adapter AND an in-memory adapter:
ui/projects/source.ts exports ProjectsWorkspaceSource plus InMemoryProjectsWorkspaceSource, and
client-admin and client-workspace follow the same shape. The in-memory source is not test
scaffolding - it is the seam that makes the controller provable without React, DOM, HTTP or the
database.

FOUR SCREENS ARE MVI-WIRED TODAY, and each component is a thin binding:
- components/portal/projects-workspace.tsx -> ui/projects/projects-controller.ts
- components/portal/clients-workspace.tsx -> ui/client-workspace/client-workspace-controller.ts
- components/portal/client-admin.tsx -> ui/client-admin/client-admin-controller.ts
- components/portal/forms/form-editor.tsx -> ui/form-editor/form-editor-controller.ts

Superseded surfaces were REMOVED rather than left running in parallel: ui/client-lens,
ui/pns-lens and ui/form-lens are gone (commit 4e610d0). Do not resurrect them.

3. THE GLASS-BOX TEST TIER (testv2/)
testv2/ imports the REAL services/ and ui/ source by relative path and is never shipped with the
application. Its point: drive a controller from node:test against a fake Source with no React, no
DOM, no HTTP and no database, then assert the PUBLISHED PageModel - including the runtime's
latest-wins, serial and parallel semantics. The projection specs are fully pure.

Run:
  node --import tsx --test testv2/*.test.ts               # no-DB service + UI tier
  node --import tsx --test testv2/engine_tests/*.test.ts   # no-DB engine tier
  pnpm test:persistence                                    # real-DEV-DB tier (env-gated)

A controller's module graph is kept alias- and runtime-clean so it runs under plain tsx. That is
a CONSTRAINT, not a coincidence: the moment a controller imports Next, React or a server-only
module, the glass-box proof stops running and the screen becomes unprovable again. Keep it clean.

4. BOUNDARIES THAT ARE NOW SETTLED - DO NOT REOPEN WITHOUT RUNTIME EVIDENCE
1. Screens do not read the database. Component -> controller -> source -> service -> repository.
2. Business truth lives in application/domain services. workflow_engine orchestrates only;
   workflow_app maps. This is now also enforced inside services/.
3. A new domain is a package under services/<domain> with its own repository interface, service,
   types and index.ts, registered in composeCoreServices. No parallel composition roots.
4. Mutations are commands: intent in, canonical service mutates, receipt/event proves it.
   Canonical relational state stays the source of truth; no full event sourcing.
5. Presentation state belongs to the UI tier; concurrency and resolution belong to the runtime,
   not to a component's useEffect.
6. Conditional UI is derived from available data. No listing-specific special-casing.
7. A fixture is NEVER a runtime fallback. The design fixture is test/prototype input only -
   PROJECTS-MVI-01 states this as an acceptance criterion, and a read failure must surface as an
   explicit unavailable state instead.

5. HONEST GAPS AS OF 2026-09-11/12 (do not report these as done)
- Only four portal screens are MVI-wired. Most of /portal still reads through older paths, so the
  tier and the legacy surfaces currently coexist. Migrating the rest is unplanned work.
- The architecture hard gate can read clean WITHOUT RUNNING. dependency-cruiser and knip are not
  installed in this repo (semgrep and ripwire are), so StaticGateResult.archRan is false and the
  correct reading is INCOMPLETE, not PASS. Adding those two as devDependencies is required and
  NOT done.
- Deployment receipts (releaseEvidence) are never populated, so the deploy stage HOLDs on
  devops-receipt. That is TECH-DEBT-07, and it also blocks PROJECTS-WORKSPACE-14.
- The contract domain is not yet released in PROD.

6. REFERENCES
- ARCH-HANDOFF (this row) - architecture, boundaries, operating model. Read FIRST.
- DEEP1 - data pipeline doctrine (ODS -> warehouse -> screen) and its traps. Read SECOND.
- SOP1 - factory/queue health doctrine. Read when judging Forge health.
- Packets: docs/agent/packets/PROJECTS-MVI-01.md, PROJECTS-MVI-02.md.
- Design: docs/catchup-wbs-design.md. Pattern statement: testv2/README.md.
- Memory: docs/agent/MEMORY.md (decision log), docs/agent/FORGE-WORKSHOP.md (boot sheet).

A later explicit decision from Chris supersedes this amendment and must be durably recorded.`

const ARCH_NOTES_AMENDMENT = `

[2026-09-11/12 architect session] AMENDED WITH THE SERVICE TIER AND THE MVI PAGE RUNTIME. The brief above predated both: the services/ kernel (14 domains, ONE composition root, four mandatory infrastructure ports, envelopes whose expected failures are audited control flow rather than error rows) and the ui/runtime MVI runtime (BasePageController; per-operation parallel/latest/serial execution; pure projections; a Source interface per screen with a real AND an in-memory adapter) landed 2026-09-06..09-11. Four portal screens are wired through them - projects, clients, client-admin, form-editor - and testv2/ proves their controllers glass-box with no React/DOM/HTTP/DB. Two corrections made in the same pass: (a) the stored brief had two blocks holding LITERAL backslash-n instead of newlines, which made the QUICK ORIENTATION invariants render as one unreadable line - repaired; (b) priority normalised from "REFERENCE" to "Reference" so the three reference rows group together on the board. The three reference stories, read in this order: ARCH-HANDOFF, then DEEP1 (data pipeline), then SOP1 (factory health).`

const SOP_AMENDMENT = `

================================================================================
${SOP_MARKER} - WHAT CHANGED UNDER THIS DOCTRINE
================================================================================

The doctrine above is UNCHANGED and still correct: one failed story is not a failed factory,
understood residue is suppressed, replenishment is independent of isolated failure. These are the
operating facts added since it was written, and each one changes how a watch is conducted.

1. FORGE RUNS AGAINST PROD ONLY. NEVER DEV.
Captain's directive, meant literally: "we are NEVER running forge against DEV again." Forge runs -
engine lanes, dogfoods, splits, role attempts - execute against PROD only. DEV is for
application/dev work and for hand-run scripts; it is NOT a Forge execution target. WHY, so the rule
survives its author: the WS series ran in DEV while the Story Board lives in PROD, so shipping
twelve stories left the board unable to show its own numbers and the board silently disagreed with
git - a morning lost reconciling rows by hand. Consequences a watcher must enforce:
(a) a run whose resolved target is not PROD is a DEFECT; the guard belongs at run start and must
FAIL CLOSED, not warn;
(b) if a run ever does happen elsewhere, its execution_environment must be VISIBLE ON THE BOARD so
the mismatch can never masquerade as PROD evidence;
(c) do NOT close a resulting history gap by re-running work - recover it with pnpm forge:sync-history
(scripts/sync-forge-history.ts), which copies stories -> runs -> work items additively and
idempotently (on conflict (id) do nothing; never an update, never a delete).

2. TWO RELEASE GATES, AND THEY ARE GATES, NOT REPORTS.
pnpm db:parity (scripts/check-schema-parity.ts) and pnpm db:migrations (scripts/migration-status.mjs)
are release gates. A Neon branch reset HIDES drift rather than fixing it, so parity must be checked
independently of any reset. Migration 144 added the schema_migration ledger, which records every
apply with target, checksum and note, so "what was run where" is finally answerable; pre-baseline
history is reported as UNRECORDED rather than claimed. On 2026-09-10 DEV and PROD had silently
diverged in BOTH directions for weeks - PROD never received 116-122/138, DEV never received the Forge
dispatch columns, and migration 118's rename was only half-reflected in code. Assume divergence is
the default until parity says otherwise.

3. THE ARCHITECTURE HARD GATE CAN PASS WITHOUT RUNNING.
dependency-cruiser and knip are NOT installed in this repo; semgrep and ripwire ARE. With the tool
absent the gate skips by design, so StaticGateResult.archRan is false and the honest reading is
INCOMPLETE - never "architecture clean". Verify a tool's status by EXECUTING it: never from a status
doc, and never from the presence of an import or a code path. "A code path exists" is not "the tool
runs." pnpm forge:tools (scripts/forge-tools.ts) exists so a human or an agent can interrogate and
RUN the analyzer tools directly instead of trusting a doc.

4. DEPLOY RECEIPTS ARE STILL DARK (TECH-DEBT-07).
AgentRunEvidence.releaseEvidence is never populated, so the deploy stage HOLDs on devops-receipt: a
story can publish and still not be RECORDED as deployment-verified. That is a known blocker, not a
story failure - do not classify it as one, and do not fabricate a receipt to clear it. A fabricated
receipt is worse than a HOLD.

5. THE SPLIT DOOR IS OPEN.
FORGE_SPLIT_ENABLED now defaults ON (only the literal "false" turns it off); FORGE_SPLIT_MAX_SMITHS
defaults 2 and FORGE_SPLIT_CONCURRENCY defaults 2. Each child's candidate is diffed from its merge
base and refused if any changed path falls outside allowedScope or inside prohibitedScope, so a
fan-out is scope-verified rather than merely SHA-collecting. Consequence for a watch: a SPLIT story
legitimately occupies more than one slot, and a child that visibly edits outside its lane is a real
defect worth reporting.

6. WHAT A HEALTHY REPORT LOOKS LIKE NOW
Lead with factory health and current progress, then isolated failures, then understood residue, then
queue counts and the replenishment decision - the same order as above, now stating the ENVIRONMENT of
each run, because under a PROD-only regime the environment is the load-bearing fact.`

const SOP_NOTES_AMENDMENT = `

[2026-09-11/12 operator session] AMENDED. The queue-health doctrine is unchanged; the operating facts beneath it are not. Added: Forge runs against PROD ONLY, never DEV (fail-closed guard at run start, execution_environment visible on the board, history gaps recovered with pnpm forge:sync-history rather than by re-running work); pnpm db:parity and pnpm db:migrations are RELEASE GATES, and parity must be checked independently because a branch reset hides drift; the architecture hard gate can read clean WITHOUT RUNNING (dependency-cruiser and knip are not installed - the correct reading is INCOMPLETE, never PASS); deploy receipts are still dark under TECH-DEBT-07, so a HOLD there is a known blocker rather than a story failure and a fabricated receipt is worse than a HOLD; the SPLIT door is open by default with scope-verified children.`

type Execute = NonNullable<Parameters<typeof getStoryboardStory>[1]>

/**
 * Two blocks of the ARCH-HANDOFF brief were stored with the two-character sequence backslash-n
 * instead of real newlines, so the read-this-first invariants block rendered as one unreadable
 * line. Repair it, and report what was repaired rather than doing it silently.
 */
function unescapeLiteralNewlines(text: string): { text: string; repairs: number } {
  const repairs = text.split('\\n').length - 1
  if (repairs === 0) return { text, repairs: 0 }
  return { text: text.split('\\n').join('\n'), repairs }
}

function occurrences(haystack: string, needle: string): number {
  return haystack.split(needle).length - 1
}

async function updateArchHandoff(execute: Execute): Promise<void> {
  const id = 'ARCH-HANDOFF'
  const story = await getStoryboardStory(id, execute)
  if (!story) throw new Error(`${id} not found`)

  const briefBefore = story.architectBrief ?? ''
  const { text: repaired, repairs } = unescapeLiteralNewlines(briefBefore)

  // Insert the new invariants only when the anchor is unambiguous (exactly one occurrence), so a
  // repeated phrase can never land them in the wrong place.
  let withInvariants = repaired
  if (!repaired.includes('THREE REFERENCE STORIES, READ IN THIS ORDER')) {
    const hits = occurrences(repaired, INVARIANT_ANCHOR)
    if (hits === 1) {
      withInvariants = repaired.replace(INVARIANT_ANCHOR, NEW_INVARIANTS)
    } else {
      console.log(`  invariants: SKIPPED (anchor matched ${hits} times, expected exactly 1)`)
    }
  } else {
    console.log('  invariants: already present, left as-is')
  }

  const appendixAlready = withInvariants.includes(MVI_MARKER)
  const briefAfter = appendixAlready ? withInvariants : withInvariants + ARCH_AMENDMENT
  const notesBefore = story.notes ?? ''
  const notesAfter = notesBefore.includes('AMENDED WITH THE SERVICE TIER AND THE MVI PAGE RUNTIME')
    ? notesBefore
    : notesBefore + ARCH_NOTES_AMENDMENT

  const priorityBefore: string = story.priority
  if (briefAfter === briefBefore && notesAfter === notesBefore && priorityBefore === 'Reference') {
    console.log(`${id}: already current - nothing to write`)
    return
  }

  await updateStoryboardStory(
    id,
    {
      workstream: story.workstream,
      title: story.title,
      priority: 'Reference',
      status: story.status,
      notes: notesAfter,
      batch: story.batch,
      goal: story.goal,
      scope: story.scope,
      dependencies: story.dependencies,
      preconditions: story.preconditions,
      architectBrief: briefAfter,
      contextRefs:
        'READ FIRST; architecture handoff; ultimate README; continuity record; in-case-of-death handoff; then DEEP1 (data pipeline) then SOP1 (factory health)',
      acceptanceCriteria: story.acceptanceCriteria,
      postconditions: story.postconditions,
      completion: story.completion,
      rollup: story.rollup,
      plannedStartAt: story.plannedStartAt,
      actualStartAt: story.actualStartAt,
      completedAt: story.completedAt,
      operatingSurface: story.operatingSurface,
    },
    execute,
  )

  console.log(`${id}: updated`)
  console.log(`  priority: ${priorityBefore} -> Reference`)
  console.log(`  brief: ${briefBefore.length} -> ${briefAfter.length} chars (literal backslash-n repairs: ${repairs})`)
  console.log(`  mvi amendment appended: ${!appendixAlready}`)
  console.log(`  notes: ${notesBefore.length} -> ${notesAfter.length} chars`)
}

async function updateSop1(execute: Execute): Promise<void> {
  const id = 'SOP1'
  const story = await getStoryboardStory(id, execute)
  if (!story) throw new Error(`${id} not found`)

  const briefBefore = story.architectBrief ?? ''
  const briefAfter = briefBefore.includes(SOP_MARKER) ? briefBefore : briefBefore + SOP_AMENDMENT
  const notesBefore = story.notes ?? ''
  const notesAfter = notesBefore.includes('AMENDED. The queue-health doctrine is unchanged')
    ? notesBefore
    : notesBefore + SOP_NOTES_AMENDMENT

  if (briefAfter === briefBefore && notesAfter === notesBefore) {
    console.log(`${id}: already current - nothing to write`)
    return
  }

  await updateStoryboardStory(
    id,
    {
      workstream: story.workstream,
      title: story.title,
      priority: story.priority,
      status: story.status,
      notes: notesAfter,
      batch: story.batch,
      goal: story.goal,
      scope: story.scope,
      dependencies: story.dependencies,
      preconditions: story.preconditions,
      architectBrief: briefAfter,
      contextRefs: story.contextRefs,
      acceptanceCriteria: story.acceptanceCriteria,
      postconditions: story.postconditions,
      completion: story.completion,
      rollup: story.rollup,
      plannedStartAt: story.plannedStartAt,
      actualStartAt: story.actualStartAt,
      completedAt: story.completedAt,
      operatingSurface: story.operatingSurface,
    },
    execute,
  )

  console.log(`${id}: updated`)
  console.log(`  brief: ${briefBefore.length} -> ${briefAfter.length} chars`)
  console.log(`  notes: ${notesBefore.length} -> ${notesAfter.length} chars`)
}

async function main() {
  const url = process.env.DATABASE_URL_PROD
  if (!url) throw new Error('DATABASE_URL_PROD is not set (fail closed)')
  if (url === process.env.DATABASE_URL_DEV) throw new Error('PROD URL equals DEV URL (fail closed)')

  // The Neon branch token is only visible in the HOST, and a resolved-target diagnostic alone has
  // already lied in this repo (DEEP1 section 6). Print both hosts and require them to differ.
  const prodHost = new URL(url).host
  const devHost = process.env.DATABASE_URL_DEV
    ? new URL(process.env.DATABASE_URL_DEV).host
    : '(unset)'
  console.log(`prod host: ${prodHost}`)
  console.log(`dev  host: ${devHost}`)

  const pool = createPoolExecutor(url)
  try {
    await updateArchHandoff(pool.execute)
    await updateSop1(pool.execute)
  } finally {
    await pool.end()
  }
}

main().catch((error) => {
  console.error(error instanceof Error ? error.message : String(error))
  process.exitCode = 1
})
