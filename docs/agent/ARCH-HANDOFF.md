# ARCH-HANDOFF — architecture, boundaries, operating model

> The Story Board row `ARCH-HANDOFF` (workstream ARCH, priority `Reference`, `rollup = false`)
> is generated from this file. This file is the source: it is reviewable, diffable and
> versioned, which a text column in Neon is not. Do not hand-edit the row.
>
> **Why this was rewritten (2026-09-11/12).** The row had accumulated ~53 KB of sections
> appended by successive sessions, several of them duplicated or superseded, and two blocks
> were stored with the two-character sequence `\n` instead of newlines — so the
> read-this-first invariants rendered as one unreadable 4,000-character line. Doctrine was
> preserved; the ordering, duplication and staging damage were removed. Anything now
> superseded is labelled as such rather than deleted, and volatile facts (commit hashes,
> deployment status) were moved to a dated appendix instead of being stated as timeless.

**Read order at the start of a session:**

1. **ARCH-HANDOFF** (this document) — architecture, boundaries, operating model.
2. **DEEP1** — the data pipeline (ODS → warehouse → screen) and the traps that cost real hours.
3. **SOP1** — factory / queue-health doctrine, for judging whether Forge is healthy.

Nine rows on the board carry priority `Reference`; the other six are the `ENG-FORGE-HIST-V1..V5`
Forge history records and `PORTAL-06` (information-architecture contract). They are all
deliberately `rollup = false`: reference records, never executable backlog, and they must never
create an agent work item.

---

## 0. HOW TO USE THIS DOCUMENT

Use this for **durable architecture and invariants**. Use the **live control plane** for what is
happening now.

**CURRENT RUNTIME STATE — QUERY, DO NOT MEMORIZE.** Story status, active runs, queue health,
completed work, failures and priorities are live operational facts. Retrieve them from the
current Story Board / `agent_work_item` / `storyboard_story_run` state at session start. Do not
trust any status snapshot in this document, including the dated appendices in §10.

**Fast start for a future architect:**

1. Read this handoff.
2. Query current Story Board status, active/just-completed runs, and queue health.
3. Read the architecture briefs for the stories currently in play.
4. Preserve settled boundaries unless new evidence contradicts them.
5. Distinguish observed fact from hypothesis.
6. Use OODA, and persist any material new learning.

---

## 1. CORE INVARIANTS

- Story Board = canonical specification / architecture truth.
- `agent_work_item` = durable coding command queue; `storyboard_story_run` = durable execution evidence.
- One story = one command.
- Control plane and execution plane are separate; DEV intent must never silently resolve to PROD application/domain data.
- Application/domain services own business truth and invariants; `workflow_engine` owns orchestration only; `workflow_app` owns application mapping.
- Commands express intent; canonical domain services perform mutations; receipts/events prove what happened.
- Canonical relational state remains the source of truth; no full event sourcing.
- Postgres is the default durable queue/inbox/outbox substrate until a real requirement earns external infrastructure.
- Prefer existing seams, smallest reversible changes, bounded diagnosis, fail-fast escalation, and scoped verification.
- STARTUP DELIVERY MODE: for authorized implementation, `main` is the canonical integration/Production line; the builder commits, pushes, verifies Production, and fixes forward without a side-branch handoff.
- Tests are evidence, not ritual. Full regression and `next build` require Chris's explicit authorization.
- Human judgment is reserved for architecture, security, destructive operations, visual polish, credentials, and ambiguous business semantics.
- Agents execute captured architecture; they do not casually reinvent it.
- Runtime evidence can overturn architecture; stale documentation cannot overrule fresh facts.
- Persist important learning so each failure makes the factory better.
- **The application has a typed service tier (`services/`) and an MVI page runtime (`ui/`).** A screen reaches the database through a controller → service → repository, never directly.
- **The screen is the contract.** Read the screen before inventing a mapping, and keep exactly ONE implementation of any vocabulary it declares.
- **A schema story is not delivered until DEV and PROD both match the released code.** See the Database Delivery Rule.

---

## 2. ROLE AND COLLABORATION

Act as **lead architect / engineering partner**, not a generic coding assistant. The user is a
highly experienced enterprise architect/backend engineer: do not over-explain basics, hand-hold,
or decompose everything into tutorial steps. Prefer architecture, boundaries, invariants,
sequencing, acceptance criteria and executable work orders.

**Working style**

- Architecture first, bounded execution second.
- Persist decisions so later workers do not rediscover them.
- Prefer proven patterns over novelty. Smallest useful abstraction, not speculative frameworks.
- "Prove the smallest thing / change the smallest thing / stop."
- Separate canonical truth, orchestration, integration, transport and UI.
- Long worker prompts must be one copyable code block.
- Work orders contain: goal, scope, architecture, hard boundaries, acceptance criteria,
  verification, final-report requirements.
- Do not reopen settled architecture without concrete evidence.
- Human judgment is for architecture, security, destructive operations, visual polish, provider
  credentials, and ambiguous business semantics.
- Agents should have autonomy within explicitly bounded stories.

**Tone.** Direct, technical, compact. Humor is welcome. Treat the user as a peer architect; do not
patronize. Challenge architecture when evidence warrants it, and explain the invariant or failure
mode behind the challenge rather than burying the insight under caveats. When the user says "this
reminds me of Command / MQ / Observer", investigate the structural analogy seriously — those
intuitions often contain the right architecture before the modern implementation is obvious. The
goal is not agreement; the goal is a coherent durable system.

**Human authority / working contract**

- Chris is CTO and Product Owner. Material product, information-architecture, navigation, Story
  Board or system-boundary changes require explicit approval.
- Background relevant to how he reasons: ~30 years across Lotus/IBM, JPMorgan, State Street,
  BlackRock and Royal Ahold — global Security Master, ETF publishing, CUSIP/tax-lot/real-time
  pricing, corporate actions, and a rules-based pricing engine covering roughly 1,200 stores.
- STARTUP MODE: direct small commits to `main`, Production as practical QA, rapid fix-forward, no
  branch/PR/release ceremony unless Chris asks.
- Deep reviews are read-only by default. Recommendations remain proposals until approved.
- **Product north star:** a bespoke "Pagani, not Ford" tool for Lisa, a solo luxury broker. Deep
  machinery behind a narrow interface. Automatic capture protects selling time; Lisa must never
  become a data-entry operator.

A good architecture session leaves behind: a clearer model, a durable decision, a smaller
uncertainty set, and work another capable engineer or agent can execute without reconstructing the
conversation.

---

## 3. ENGINEERING OPERATING MODEL

Use an OODA loop: **Observe → Orient → Decide → Act → Persist the learning → Repeat.**

Observed failures and runtime evidence should improve the Story Board architecture briefs,
acceptance criteria, test policy, worker rules, dispatch policy and future work orders. **A failure
should improve the factory, not merely get fixed once.** A failed story should cost one story, not
the rest of the night.

**Failure policy — bounded diagnosis + fail fast + escalate over heroics.**

Workers: (1) attempt normally; (2) inspect/reproduce once where practical; (3) try low-risk
reversible fixes inside scope; (4) do not redesign unrelated architecture to unblock; (5) escalate
on repeated failure, ambiguous root cause, missing access, security/data risk, material scope
expansion, or no-progress timeout; (6) persist blocker/evidence and release the slot safely.

Escalate when: repeated failure occurs · root cause remains ambiguous · credentials/access are
missing · the required fix materially broadens scope · architectural judgment is needed beyond
captured guidance · production/security/data-integrity risk appears · execution makes no
meaningful progress for a reasonable bounded interval.

On escalation: stop safely, preserve state, **do not falsely mark Complete**, and record the
failure, attempted fixes, likely root cause, exact blocker and recommended human action.

**Test policy**

Development/story loop: targeted tests for the changed seam; adjacent tests where justified;
typecheck where TypeScript contracts changed; build where routing/server/UI/deployment surface
changed; one real smoke test where runtime integration changed.

- **Full regression and `next build` run only with Chris's explicit authorization.** A nightly or
  pre-major-release occasion is a reason to *ask*; the label does not authorize the run.
- Do NOT reflexively run the entire regression harness after every edit. If a broader regression is
  genuinely necessary because a core invariant changed, say why before running it.
- Never run persistence/contract suites concurrently when they intentionally share a global
  single-active-resource invariant.

---

## 4. STARTUP DELIVERY / TRUNK-FIRST RELEASE DOCTRINE

**Authoritative human decision (2026-08-24).** Supersedes conflicting generic work-order language
elsewhere about feature branches, review branches, preview-only delivery, pull requests, separate
promotion gates, or a release-engineer handoff.

1. **Operating mode: STARTUP.** CulebraLuxe is in startup mode until Chris explicitly changes it.
   Optimize working V1 features in Production per unit of time and funding runway. Do not import
   Fortune 500 release ceremony into this phase — speed, short feedback loops, direct ownership and
   rapid fix-forward are intentional architecture decisions, not accidental process gaps.
2. **Authorization semantics.** Authorizing implementation, a build or a fix includes the ordinary
   end-to-end delivery mechanics: implement → smallest targeted verification → commit to `main` →
   push `main` → verify Vercel Production → fix failures caused by the change. An explicit
   read-only / diagnosis-only / context-reconstruction / no-mutation / no-push instruction still
   controls. This doctrine is never permission to mutate during a read-only task.
3. **`main` is the canonical working line.** Commit and push small completed changes frequently.
   Feature branches, review branches, preview-only releases, PRs, rebases, squashes, temporary
   integration branches/worktrees and stash choreography are not the default and require Chris's
   explicit request. A Vercel Preview may be used when Chris asks for one, but a Preview is not
   completion, and work must not remain stranded in a side-branch silo.
4. **The builder owns delivery.** Whoever changes the code owns the complete result: implementation,
   targeted tests, Git synchronization, ordinary merge/conflict resolution, commit, push to `main`,
   Vercel Production verification, and immediate correction of failures it caused. Do not hand
   integration, branch cleanup, release mechanics or deployment repair back to Chris. "Done" means
   the feature is on `main` and its Production deployment has been observed.
5. **Fix forward.** A broken build, UI defect, type error or ordinary runtime defect is feedback,
   not a reason to create release bureaucracy. Fix it immediately and push the correction; use
   rollback only when Chris directs it or it is clearly the fastest recovery. Ordinary Git
   conflicts, failed builds and correctable defects are worker responsibilities, not
   human-decision stop conditions.
6. **Test budget.** Run the smallest targeted tests and typecheck that prove the changed seam. A
   nightly label, branch merge, push to `main` or Production deployment does not by itself
   authorize the multi-hour regression harness.
7. **Valid stop conditions.** Stop for Chris only for a genuine business/architecture decision,
   missing authority or credentials, exposed credentials, or credible risk of irreversible or
   unrecoverable data loss. Do not stop merely because a change may cause an ordinary recoverable
   startup defect.
8. **Database scope.** This doctrine does not authorize unrelated or destructive database work.
   Database changes explicitly within an authorized story are owned end-to-end by the same builder:
   apply to the named target, verify, report the evidence. Read-only and environment-specific
   restrictions remain authoritative when stated.
9. **Work-order requirement.** Every future Cline, Forge, DeepSeek, Codex or other builder order for
   authorized implementation must encode this operating model. Do not add "do not push", "review
   branch only", "open a PR" or "stop before main" boilerplate unless Chris explicitly requests
   that exception.

**North-star metric:** valuable working features in Production per hour, while preserving the
settled architecture invariants and canonical data boundaries.

---

## 5. THE APPLICATION

**Read this before the older-sounding parts of any other architecture note.** Until 2026-09-06 the
portal read the database through ad-hoc loaders. It no longer does. The service tier (`services/`),
the MVI page runtime (`ui/`) and the glass-box test tier (`testv2/`) landed between 2026-09-06 and
2026-09-11, and four portal screens are wired through them end to end. Where an older document
describes a screen reaching into `db/*`, this section is what is true.

### 5.1 The service tier (`services/`)

A typed service kernel with 14 domains: `comms, contract, core, entitlement, firm, forms, person,
project, property, regrid, security, showing, vault, wbs`.

**The seam is the composition root, and it is the only one:**

```
services/composition.ts -> composeCoreServices(repositories, infrastructure)
```

It builds the kernel once, registers every domain in a `ServiceRegistry`, injects the
infrastructure, and returns typed handles (`CoreServiceComposition`): `registry`, `person`, `firm`,
`property`, `contract`, `showing`, `security`, `wbs`, `project` — plus `comms`, `form` and `vault`,
which are **optional** and present only when the composition was actually given their repository
(the source calls that "the full runtime"). Do not build a second composition root, and do not
construct a service by hand. If a domain is missing at runtime, it is missing because its
repository was not passed in.

`services/index.ts` **no longer exists** — it was removed as a dead barrel. The public seam is the
composition root plus each domain's own `index.ts`. `services/core/index.ts` is the most
depended-on file in the tier (33 dependents repo-wide), which is what keeps the kernel contracts
stable.

**Kernel contracts** live in `services/core/`:

- envelopes and results: `ServiceEnvelope`, `ServiceResult` (`ServiceSuccess | ServiceFailure`),
  `ServiceErrorShape`, `ServiceFailure`.
- operation shapes: `ServiceOperationContract`, `ServiceOperationDefinition(s)`,
  `ServiceOperationMap`, `ServiceOperationName`, `ServiceOperationKind`, `ServiceExecutionMode`,
  `ServiceExecutionPolicy`.
- identity and authority: `ServicePrincipal`, `ServiceActor`, `ServiceResourceContext`,
  `ServiceCapability`, `ServiceEndpoint`, `ServiceDescriptor`.
- `ServiceInfrastructure`: the injected ports.

**Four ports are effectively mandatory**, and the composition enforces it: `audit`
(`defaultAuditPort`), `events` (`defaultEventPort`), `errors` (`defaultErrorSink`, the
`ServiceErrorSink` seam) and `authorization`. Authorization defaults to an **enforced** resolver,
and the reason is written in the code: *"a kernel is never built without an authorization decision
source."* Do not construct a kernel that skips these ports — that is exactly how an unaudited
mutation gets shipped.

**Failure semantics inside the tier** (the same split as the repo-wide error-capture obligation):

- **Expected** business outcomes — validation failures, FORBIDDEN/authorization denials, not-found —
  are domain failures returned in the envelope. They are audited control flow, **not** error rows.
- Anything **unhandled** is captured through the errors sink with domain, operation and
  `correlationId`. Never a bare `try/catch`, never `console.error` alone, never a silent 500.

**Repository boundary rule.** Repository boundaries own normalization of driver-native values into
stable application contracts. Nothing above the repository should ever see a `Date`, `BigInt`,
`Buffer` or driver-specific object from Neon/Postgres.

**A component importing from `db/*` is a defect**, and it is now machine-checked: commit `f1bc0a3`
fixed the 6 component→db violations the new architecture gate found.

### 5.2 The MVI screen runtime (`ui/`)

`ui/runtime/` is a transport- and framework-neutral **Model-View-Intent** page runtime. Its reason
to exist is stated in the file itself: dispatch ingress, model publication and concurrency
semantics *"otherwise get reimplemented in React components."*

- `BasePageController<TModel, TMap> implements PageStore<TModel>`. A concrete controller declares
  exactly one thing the runtime cares about: `operations: PageOperationDefinitions<TModel, TMap>`.
- The parent owns dispatch, publication and concurrency; the controller owns what each intent does.
  The protected helpers `replaceModel`/`updateModel` are how a controller publishes, and `dispose()`
  aborts every in-flight operation and stops publication.

**Three execution modes, declared per operation** (`PageOperationDefinition.execution`):

- **`parallel`** (the default) — the operation gets its own `AbortController` and an `isCurrent()`
  gate before any model write.
- **`latest`** — LATEST-WINS. Dispatching the same operation again **aborts** the previous
  in-flight one. This is the search-as-you-type / filter / paging case; do not hand-roll it in a
  component.
- **`serial`** — per-operation chaining (`serialTails`), so an operation can never interleave with
  itself even when dispatched twice in a row.

**Context handed to a handler** (`PageOperationContext`): `signal`, `snapshot()`, `update(reducer)`,
`isCurrent()`. An operation that writes the model after it has been superseded is exactly the bug
class the runtime exists to prevent — guard writes with `context.update()`, which already checks
disposed/aborted/isCurrent, instead of setting state directly.

**React never drives state.** The React-facing contract is `PageStore`: `snapshot()` +
`subscribe()`, bound through `ui/runtime/use-page-controller.ts`. Components render the published
model and dispatch intents; nothing else.

**Projections are pure** and separate from both the source and the controller:
`ui/projects/service-projection.ts` (`mapRealProjectsToWorkspace`), `ui/projects/tree-projection.ts`,
`ui/client-workspace/channel-projection.ts`. A projection maps service DTOs into a page model. It
does not fetch, and it does not import React.

**Every screen has a Source interface with a real adapter AND an in-memory adapter.**
`ui/projects/source.ts` exports `ProjectsWorkspaceSource` plus `InMemoryProjectsWorkspaceSource`;
`client-admin` and `client-workspace` follow the same shape. The in-memory source is not test
scaffolding — it is the seam that makes the controller provable without React, DOM, HTTP or the
database.

**Four screens are MVI-wired today**, and each component is a thin binding:

| Component | Controller |
|---|---|
| `components/portal/projects-workspace.tsx` | `ui/projects/projects-controller.ts` |
| `components/portal/clients-workspace.tsx` | `ui/client-workspace/client-workspace-controller.ts` |
| `components/portal/client-admin.tsx` | `ui/client-admin/client-admin-controller.ts` |
| `components/portal/forms/form-editor.tsx` | `ui/form-editor/form-editor-controller.ts` |

Superseded surfaces were **removed** rather than left running in parallel: `ui/client-lens`,
`ui/pns-lens` and `ui/form-lens` are gone (commit `4e610d0`). Do not resurrect them.

### 5.3 The glass-box test tier (`testv2/`)

`testv2/` imports the **real** `services/` and `ui/` source by relative path and is never shipped
with the application. Its point: drive a controller from `node:test` against a fake Source with no
React, no DOM, no HTTP and no database, then assert the **published PageModel** — including the
runtime's latest-wins, serial and parallel semantics. The projection specs are fully pure.

```
node --import tsx --test testv2/*.test.ts               # no-DB service + UI tier
node --import tsx --test testv2/engine_tests/*.test.ts   # no-DB engine tier
pnpm test:persistence                                    # real-DEV-DB tier (env-gated)
```

A controller's module graph is kept alias- and runtime-clean so it runs under plain `tsx`. That is
a **constraint**, not a coincidence: the moment a controller imports Next, React or a server-only
module, the glass-box proof stops running and the screen becomes unprovable again. Keep the graph
clean.

### 5.4 Boundaries that are settled — do not reopen without runtime evidence

1. **Screens do not read the database.** Component → controller → source → service → repository.
2. **Business truth lives in application/domain services.** `workflow_engine` orchestrates only;
   `workflow_app` maps. This is now also enforced *inside* `services/`.
3. **A new domain is a package under `services/<domain>`** with its own repository interface,
   service, types and `index.ts`, registered in `composeCoreServices`. No parallel composition roots.
4. **Mutations are commands:** intent in, canonical service mutates, receipt/event proves it.
   Canonical relational state stays the source of truth; no full event sourcing.
5. **Presentation state belongs to the UI tier**; concurrency and resolution belong to the runtime,
   not to a component's `useEffect`.
6. **Conditional UI is derived from available data.** No listing-specific special-casing.
7. **A fixture is never a runtime fallback.** The design fixture is test/prototype input only —
   `PROJECTS-MVI-01` states this as an acceptance criterion, and a read failure must surface as an
   explicit unavailable state instead.

### 5.5 The data pipeline, and the paging standard

**The pipeline doctrine lives in DEEP1 — read it second, after this document.** Summary of the rule
that matters most: **ODS** is the `l_*` tables (raw intake, written by intake scripts, write-only,
and **nothing client-facing may ever read one**); the **warehouse** holds only what the screen
contract needs, cherry-picked out of ODS by promotion scripts, which are the only code permitted to
read an `l_` table; the **screen** reads the warehouse through a service (`services/*`), never
through a repository inline. L keeps everything, which is what makes the warehouse safe to be lossy:
every warehouse row is re-derivable from L. DEEP1 also carries the warehouse grain rule, the
round-trip-cost law, the identity traps and the database-target trap.

**Data surface / paging standard — authoritative human decision (2026-08-25).** Real Apple Messages
evidence exposed a demo-scale assumption: `/portal/clients` hung with 2,351 persisted
relationship-evidence rows. This is an application-wide invariant.

Every potentially unbounded data collection displayed by a UI must have a **bounded read contract**:
server-side paging; filter/search/sort applied in SQL or the backing datastore *before* the page
window; counts retrieved separately when needed; enrichment kept set-based/batched; and inactive
large tabs/panes must not block active content. Default page size is 50 unless a surface explicitly
justifies another bound. Prefer keyset/cursor pagination; measured `LIMIT/OFFSET` is acceptable for
ordinary navigation.

Do **not** hydrate entire growing datasets in application memory merely to render/search/sort/count a
screen. Do not raise timeouts, or delete/hide real rows, as a substitute for fixing read-path scale.
New data screens inherit this standard automatically; existing screens are corrected as they are
touched or when real volume exposes a defect. See `ENG-34` for executable acceptance criteria.

### 5.6 The Contact / CRM spine and the macOS integration edge

External activity should resolve toward canonical person/contact identity, then become CRM
interaction/timeline/business actions. The conceptual spine:

```
Person/Contact -> email/messages/calendar/calls/notes -> interaction timeline -> deal/task/workflow relationships
```

**Do not create five unrelated integration models.**

The Mac is the integration edge, and inbound external activity is the mirror image of an outbox:

```
macOS source observer/adapter -> ExternalActivityEvent -> Durable Integration Inbox
  -> identity/contact resolution -> mapper -> existing CRM intake/interaction seam
  -> Business Command -> canonical CRM truth
```

Potential observers: Contacts, Calendar, Mail, Messages, WhatsApp, later calls/notes/other sources.
**The observer is dumb:** it reports what happened externally and does not decide business
consequences. Source-specific API ugliness must stay below the adapter/observer boundary.

```
INBOX   = durable facts coming IN
COMMAND = requested CRM action
DOMAIN  = canonical truth
OUTBOX  = durable facts going OUT
```

For the person ↔ property model in detail see `docs/agent/PERSON-PROPERTY-DESIGN.md` (the five-line
picture and the four rules R1–R4: one direction, newer wins, identity = phone or email, promote
everything).

---

## 6. THE SOFTWARE FACTORY

### 6.1 Canonical pattern

```
Story Board
  -> durable agent_work_item command
  -> poller/invoker
  -> AgentRuntimeAdapter
  -> concrete runtime adapter
  -> model / tools
  -> evidence
  -> storyboard_story_run / terminal work item
```

- **Story Board** = specification / architecture truth.
- **`agent_work_item`** = durable command queue.
- **poller/invoker** = command invoker.
- **`AgentRuntimeAdapter`** = execution abstraction; the concrete receiver has been
  `DeepSeekHarnessAdapter` and is now also other harnesses.
- **`storyboard_story_run`** = durable evidence / history.

**One story = one command.** The factory must remain runtime-neutral above the adapter boundary.
Logical model profiles such as `builder-flash` / `architect-pro` must not leak provider-specific
model names into canonical command semantics.

### 6.2 Control plane vs execution plane

The separation is architectural and permanent:

- **Control plane may be PROD:** Story Board, `agent_work_item`, `storyboard_story_run`,
  evidence/accounting.
- **Execution plane may be DEV/local:** local repo, DEV Neon application DB, local processes, the
  model harness.

**Never infer the execution target from the location of the control-plane row.** DEV execution
resolving to the PROD application/domain DB must fail fast **before work begins**, and there must be
no silent generic `DATABASE_URL` fallback from DEV intent to PROD.

> **2026-09-11 — the mandated Forge target.** Chris's directive: *"we are NEVER running forge
> against DEV again."* Forge **runs** (engine lanes, dogfoods, splits, role attempts) execute against
> **PROD only**; DEV is for application work and hand-run scripts. A run whose resolved target is not
> PROD is a **defect**, the guard belongs at run start and must **fail closed**, and a run's
> `execution_environment` must be **visible on the board** so a mismatch can never masquerade as PROD
> evidence. History gaps are recovered with `pnpm forge:sync-history` (additive, idempotent) — never
> by re-running work. See **SOP1** for the operator doctrine, and DEEP1 §6 for the database-target
> trap that makes "the resolved target" a diagnostic you must not trust without printing the
> connection host.

### 6.3 Workflow engine boundary

**Application owns canonical business truth.**

`workflow_engine` owns orchestration: tokens, transitions, timers/jobs, retries, fork/join, generic
human-task mechanics, terminal semantics.

`workflow_app` owns: workflow definitions, `ApplicationPort`, facts, business command routing,
task/workflow correlation, application-facing workflow reads.

**The engine must not know CRM/domain tables.**

**Proven capabilities — do not replace the engine with a new orchestration framework unless evidence
requires it:** real Postgres persistence · concurrent joins · transactional atomicity · deterministic
command replay · idempotency · retries/failure handling · timer/lease/requeue · operational recovery
seams.

### 6.4 Business command architecture

Before significant additional CRM→workflow mapping, establish a canonical Business Command layer.

```
UI / Workflow / API / Agent
  -> BusinessCommand
  -> CommandDispatcher
  -> CommandHandler
  -> canonical domain/application service
  -> canonical mutation
  -> durable command receipt
  -> normalized result
  -> optional future domain events
```

The **command layer** owns generic concerns: `commandId`, command type, actor/context,
aggregate/entity identity, `correlationId`, `causationId`, execution timestamp, replay/idempotency,
normalized result, receipt persistence, transaction orchestration where appropriate.

**Command handlers must remain thin.** Business legality and invariants stay in canonical domain
services.

```
COMMAND = INTENT
DOMAIN  = TRUTH + BUSINESS RULES
RECEIPT = PROOF
```

Existing command/replay infrastructure should be **generalized, not replaced**.

### 6.5 Domain event / messaging direction

```
successful command -> canonical mutation -> command receipt
  -> domain event persisted in the SAME database transaction -> commit
then:
Outbox -> dispatcher -> subscriber(s) -> delivery receipt / retry / escalation
```

```
EVENT      = FACT THAT ALREADY HAPPENED
OUTBOX     = DURABLE HANDOFF
SUBSCRIBER = REACTION
```

**Postgres is the V1 durable messaging substrate.** Borrow MQ semantics: durable delivery,
at-least-once, subscriber idempotency, retries, correlation, replay, dead-letter/escalation
concepts.

Do **not** implement full event sourcing. Canonical relational state remains truth. Do not rebuild
aggregates from event history. Do not introduce Kafka/RabbitMQ merely for architectural aesthetics —
add external broker infrastructure only when scale/fan-out/stream semantics justify it. If Postgres
can safely provide the semantics today, use Postgres. If one local worker is enough today, use one
local worker.

**Do not confuse a pattern with its implementation.** Command is not a TypeScript class. Messaging is
not RabbitMQ. Workflow is not an XML file. An event is not Kafka. A queue is not a product. An adapter
is not a vendor. *Architecture is the set of invariants that remain true when implementations change.*

### 6.6 Architecture stories vs implementation stories

**Architecture briefs** define: what truth exists · who owns it · boundaries/invariants · rejected
designs · sequencing. **Implementation stories** define: interfaces/contracts · classes/stubs · exact
service mappings · migrations · targeted tests · execution/reporting.

**Paid architecture analysis should be reused, not redone.**

### 6.7 Story / work-order quality bar

Good work orders incorporate actual observed failures and architecture lessons, not generic
boilerplate. Always specify: explicit goal · architecture boundary · what is canonical truth · what
layer owns what · environment boundaries · a hard "DO NOT" list · scoped verification policy ·
evidence required in the final report · stop condition. For risky stories, say explicitly what the
worker is **not** authorized to do.

The real chain has been proven end to end (**ENG-19**: Story Board → durable work item → invoker →
`AgentRuntimeAdapter` → real harness → model → repo/tools/tests → local commit → normalized evidence
→ terminal run/work item/story). **Do not re-prove ENG-19 casually.**

### 6.8 Forge operations

For anything about judging whether the factory is healthy — one failed story vs a wedge, residue,
replenishment, escalation triggers, the PROD-only rule, the `pnpm db:parity` / `pnpm db:migrations`
release gates, the analyzer-tool boundary, and the dark deploy receipts — read **SOP1** rather than
this section.

---

## 7. SAMPLE WORK ORDER FORMAT — CULEBRALUXE ENGINEERING FACTORY

> Doctrine says a long worker prompt must be **one copyable code block**. The original template was
> stored with every bullet exploded onto its own line by the tooling that wrote it; this is the same
> guidance, copyable.

```
TITLE
  IMPLEMENT STORY <STORY-ID> — <TITLE>

ROLE
  You are the builder for this story.
  Read <STORY-ID> from the canonical Story Board first.
  Treat these Story Board fields as authoritative: goal, scope, dependencies, preconditions,
  architect_brief, context_refs, acceptance_criteria, postconditions, notes.
  Do not substitute your own architecture where the Story Board already defines one.

GOAL
  <One concise paragraph: the smallest complete slice that satisfies the business/engineering
  objective while preserving existing architecture, canonical truth ownership and runtime
  invariants. The result must be objectively verifiable.>

CURRENT STATE / KNOWN FACTS
  Only facts already established by prior work or inspection:
    - existing canonical service: <path>
    - existing runtime seam: <path>
    - migration already present: <migration>
    - prior story <ID> proved <capability>
    - environment distinction: control plane = <PROD/DEV>, execution target = <DEV/LOCAL>
    - existing tests covering adjacent behavior: <paths/counts>
  Do not re-prove settled architecture unless new evidence contradicts it.

ARCHITECTURE
  Define the exact intended flow, e.g.
    Caller -> Command -> Dispatcher -> Handler -> Canonical Domain Service -> Repository -> Receipt
    External Source -> Observer Adapter -> Durable Inbox -> Mapper -> Business Command -> Domain Truth
  State ownership explicitly:
    - workflow_engine owns orchestration only
    - workflow_app owns application mapping
    - the domain service owns business invariants
    - the repository owns persistence mechanics
    - the adapter owns vendor-specific mechanics
    - the UI is a projection, not canonical state
  Define transaction boundaries, what is canonical truth, and which existing abstractions must be
  reused.

IMPLEMENTATION SCOPE
  The exact build slice. Prefer concrete files/interfaces/classes where architecture is mature:
    1. Add/extend <interfaces/classes>
    2. Wrap existing canonical services <...>
    3. Preserve existing service behavior
    4. Add only the smallest schema change required
    5. Add targeted tests for the new seam
  Do not build future phases merely because the abstraction could support them.

INVARIANTS
  Non-negotiable:
    - canonical relational state remains source of truth
    - a stable commandId makes replay idempotent
    - duplicate execution must not duplicate business effects
    - domain rules must not move into the infrastructure layer
    - subscriber failure must not roll back committed domain truth
    - a DEV command must never silently resolve to the PROD application DB
    - a terminal workflow instance must not be resurrected
    - one story = one durable agent command
  If an implementation choice conflicts with an invariant, STOP and report it.

HARD BOUNDARIES — DO NOT
    - redesign unrelated architecture
    - create a second queue
    - create a second canonical state model
    - bypass canonical domain services
    - move business rules into the workflow engine
    - introduce full event sourcing
    - introduce external infrastructure without requirement
    - touch production application/domain data unless explicitly authorized
    - during read-only/diagnosis work: push or deploy. (During authorized implementation, delivery
      to main and Production is implied unless Chris explicitly prohibits it.)
    - run giant unrelated regression suites
    - perform visual polish unless requested
    - broaden scope to fix unrelated issues
  If blocked by something outside scope: bounded diagnosis, then escalate.

FAIL-FAST / ESCALATION POLICY
  Attempt the task normally. If blocked: inspect relevant logs/state; reproduce once if practical;
  identify the likely root cause; try only low-risk reversible fixes within scope.
  Escalate when: repeated failure occurs; root cause remains ambiguous; credentials/access are
  missing; the required fix materially broadens scope; architectural judgment is required beyond
  captured guidance; production/security/data-integrity risk appears; execution makes no meaningful
  progress for a reasonable bounded interval.
  On escalation: stop safely; preserve state; do not falsely mark Complete; record the failure,
  attempted fixes, likely root cause, exact blocker, recommended human action.
  PRINCIPLE: a failed story should cost one story, not the rest of the batch.

TEST POLICY
  RUN SCOPED TESTS ONLY. Full regression or next build requires Chris's explicit authorization,
  including at a promotion boundary.
    - targeted tests for the changed seam
    - adjacent tests where justified
    - typecheck if TypeScript contracts changed
    - build if routing/server/UI/deployment surface changed
    - one real smoke test if runtime integration changed
  DO NOT reflexively run the entire regression harness. Never run persistence suites concurrently
  when they intentionally share a global single-active-resource invariant.

ENVIRONMENT / DATA SAFETY
  State explicitly:
    CONTROL PLANE:     <environment>
    EXECUTION TARGET:  <environment>
    APPLICATION DB:    <environment>
    Allowed writes:    <exact scope>
    Forbidden writes:  <exact scope>
  Any environment mismatch that could write DEV-intended work to PROD must FAIL BEFORE WORK BEGINS.
  Never rely on ambiguous fallback configuration for destructive or stateful operations.

ACCEPTANCE CRITERIA
  Make each item observable, e.g.
    1. the new command executes through the canonical dispatcher
    2. the existing domain service remains the mutation authority
    3. a duplicate commandId returns the prior result and performs no duplicate mutation
    4. correlationId and causationId persist through the receipt
    5. targeted tests pass
    6. typecheck passes
    7. the working tree contains only intended changes
    8. no production data changed
    9. evidence identifies exact files/tests/commit
   10. the Story Board result accurately reflects success or blocker
  Avoid vague criteria ("works correctly", "looks good", "is robust").

EXECUTION SEQUENCE
  1. Inspect existing seams first.
  2. Confirm architecture assumptions against code.
  3. Implement the smallest structural change.
  4. Add targeted proof.
  5. Run scoped verification.
  6. Fix only failures caused by this story.
  7. For authorized implementation, commit to main, push, and verify Vercel Production unless Chris
     explicitly prohibits it.
  8. Update durable evidence.
  9. STOP. Do not continue into adjacent backlog stories.

FINAL REPORT
  Story implemented; architecture actually used; files changed; schema/migrations changed; canonical
  services reused; new contracts/interfaces/classes; invariants proved; exact scoped tests run and
  results; typecheck/build result if applicable; runtime/smoke evidence if applicable; environment
  used; DB writes performed; confirmation forbidden environments were untouched; git commit hash(es);
  working-tree status; residual risks; deferred work explicitly not implemented; final verdict:
      GREEN
      or
      BLOCKED — <exact reason>
  Then STOP. Do not start the next story.
```

---

## 8. ARCHITECTURE CONTINUITY — RECONSTRUCTION EVIDENCE STANDARD

**Recorded 2026-08-24, from a clean-room architecture reconstruction.** Supplements the invariants
above; it does not replace them.

1. **Resolve the durable target before writing.** Informal architecture labels may not equal literal
   Story Board IDs. Resolve the actual record by exact ID and title before mutating. If no record can
   be proven, report the ambiguity and request a human decision — do not invent a replacement row or
   silently update a likely match.
2. **Report contradictions before interpreting them.** Follow the authority/read order, then compare
   every durable claim against current Git topology, production composition roots, migrations and the
   canonical database. A read order does not authorize silently rewriting contradictions: report the
   conflict, the evidence on each side, and the smallest decision needed. Do not repair a
   contradiction by introducing a second workflow, queue, event store, Deal owner or Deal-state
   model.
3. **A committed packet is not enough — the stable pointer must reach it.** A continuity packet
   committed only on a side branch is not a complete handoff if the stable entry document on the
   canonical branch cannot lead a new session to it. Preserve historical packets, but update the
   stable pointer and its successor packet together. A clean-room reader should be able to start from
   the durable entry point and discover the current packet without prior chat memory.
4. **Distinguish infrastructure existence from production composition.** The existence of a contract,
   table, migration, repository, event type or injectable test seam does **not** prove production uses
   it. Inspect every production composition root and verify the concrete dependency actually supplied
   at runtime. For transactional publication, prove all of: the production dispatcher receives the
   real Postgres outbox repository; aggregate mutation, command receipt and outbox insert share one
   database transaction; rollback leaves none of those writes committed; the emitted type is
   supported by the persisted outbox contract and its consumer path. A TypeScript union member, a
   should-emit flag, an in-memory fake sink or a passing unit test is **design evidence**, not proof
   of durable publication.
5. **Reconcile Git and database topology explicitly.** Inspect all relevant local and remote branches,
   their merge bases, and committed-but-unmerged continuity artifacts. Do not infer ancestry from
   commit dates or story status. Distinguish "migration committed in Git" from "migration applied" on
   each named Neon branch. Identify the canonical Neon project through **schema fingerprints**
   (beginning with `storyboard_story` and the expected architecture records), not a remembered project
   name, ID or connector list. A connector authorization failure is not evidence that the database is
   absent or unhealthy.
6. **Label evidence and avoid overclaiming.** Classify material findings as **observed, inferred or
   unverified**, and state the exact evidence boundary. Absence of a direct version key is not proof
   that version scope is absent — follow the foreign-key lineage. Deployment-platform failures are
   operational evidence; they become architecture blockers only when evidence causally ties them to a
   branch's build, typecheck, migration or runtime composition.
7. **Story status, branch content and tests are different signals.** Story Board status can lag
   partial or corrective work. Report both the durable story state and the actual branch diff.
   Readiness must rest on the branch's concrete implementation, scoped tests, build/typecheck,
   production wiring and unresolved architecture decisions — not status alone. Tests prove only the
   seam they exercise; where production composition is the disputed property, include direct
   composition verification or an equivalent runtime integration proof.
8. **Minimum clean-room handoff output**, in this order: contradictions and confidence labels; Git
   state, branch topology, merge bases and stable-pointer reachability; canonical Neon project,
   branch, database, schema fingerprint and architecture-record identity; durable-document claims
   reconciled against actual production composition and applied schema; candidate-branch readiness
   separating architecture blockers from operational evidence; and only the questions that genuinely
   require human judgment.

**Record volatile commit hashes, deployment statuses and work-in-progress conclusions in a dated
continuity packet — not as timeless ARCH1 invariants.** (That is why the specific hashes that used to
appear in this section now live in §11's dated appendix instead.)

---

## 9. LISA INFORMATION ARCHITECTURE

The working lens for the portal:

```
Dashboard          = Orient
Attention/Catch-Up = Act
Client             = Remember
Deal/Contract      = Execute
```

**This is a lens, not a rigid taxonomy.** Canonical state stays normalized; Lisa's read experience may
be deliberately denormalized. Repeating a canonical fact is useful when it preserves flow or answers
a different contextual question. Remove only repetition that answers the same question in the same
context.

Do **not** recreate separate Activity or Showings destinations merely because those capabilities
exist. A panel earns its keep when it improves another panel, supports a frequent decision, prevents
a costly screen switch, stays current automatically, or justifies its visual/maintenance cost.

**Approved visible CORE vocabulary** (internal NEXUS identifiers preserved):

- Cockpit → `/portal/dashboard`
- Clients → `/portal/clients`
- Catch-Up → `/portal/attention`
- Contracts → `/portal/deals`
- Cabinet → `/portal/documents`

OPPS, SUPPORT and TECH retain their names. The portal logo returns to the public site, and the MAIN
top-nav item is removed; the public-site Portal link is the far-right final navigation item.

> **Navigation correction (2026-08-25 CTO/Product Owner) — supersedes the earlier "hidden" note.**
> Workflows (`/portal/workflows`) and Forms (`/portal/forms`) are **visible CORE secondary-navigation
> destinations**, reusing their existing screens and routes. That was a navigation-only restoration:
> no screens rebuilt, no routes changed. Any earlier statement that these remain hidden from CORE
> navigation is obsolete.

---

## 10. CURRENT STATE AND KNOWN GAPS

> **This section is a snapshot, not authority.** It is dated, and it ages. Query the live control
> plane for status; query the code and schema for architecture claims.

**As of 2026-09-11/12:**

- **Application.** The service tier and the MVI runtime exist and are proven, but **only four portal
  screens are wired through them** (projects, clients, client-admin, form-editor). Most of `/portal`
  still reads through the older paths, so the new tier and the legacy surfaces currently coexist.
  Migrating the rest is unplanned work, not a scheduled story.
- **The architecture hard gate can read clean WITHOUT RUNNING.** `dependency-cruiser` and `knip` are
  not installed in this repo (`semgrep` and `ripwire` are), so `StaticGateResult.archRan` is false and
  the correct reading is **INCOMPLETE**, never PASS. Adding those two as devDependencies is required
  and **not done**.
- **Deployment receipts are dark (TECH-DEBT-07).** `AgentRunEvidence.releaseEvidence` is never
  populated, so the deploy stage HOLDs on `devops-receipt`: a story can publish and still not be
  *recorded* as deployment-verified. This also blocks `PROJECTS-WORKSPACE-14`.
- **The `contract` domain is not yet released in PROD.**
- **Data pipeline.** Warehouse source grain is done for `apple_messages`, `apple_calls` and
  `apple_facetime`; `icloud_mail` (email) still writes one interaction per message and needs the same
  change. See DEEP1 for the full picture.
- **Board vocabulary.** `'Reference'` is now a declared priority; `'P2'` (on one row,
  `ENG-DB-RESILIENCE-01`) is still outside the declared vocabulary and was deliberately left for a
  human decision. See `docs/agent/MEMORY.md`.

**Where things stood when this was written:** PROD had **zero** projects until
`jessica-iverson-listing` was seeded (`scripts/seed-jessica-project.ts`, anchored to person
`b741d639-3173-47bc-adff-769865c6347d`), and `PROJECTS-WORKSPACE-13..18` remained Planned.

---

## 11. ACCUMULATED ENGINEERING JUDGMENT

The most valuable thing in this project is **not** any single class, table, workflow, agent or
framework. It is the accumulated judgment about where boundaries belong and how to preserve truth
under failure. Do not mistake documentation for certainty.

**When you encounter something unfamiliar:**

1. Inspect before theorizing.
2. Prefer the existing seam until evidence proves it inadequate.
3. Separate intent from fact.
4. Separate orchestration from business truth.
5. Separate source-specific ugliness from canonical application contracts.
6. Make state transitions explicit and durable.
7. Assume retries and partial failures will happen.
8. Design replay before you need replay.
9. Preserve correlation so a future operator can reconstruct what happened.
10. Never let convenience silently cross an environment or authority boundary.
11. Prefer one boring durable mechanism over several clever overlapping ones.
12. Do not build infrastructure merely because the architecture could support it.
13. When a real consumer appears, let that consumer earn the next layer.
14. Tests are evidence, not ritual.
15. A green test suite does not overrule contradictory runtime evidence.
16. A failed experiment is useful if its lesson is persisted.
17. Human attention is the scarce resource; spend it on judgment, not repetitive mechanics.
18. Agents should execute architecture, not continuously reinvent it.
19. Architecture should constrain implementation enough to make autonomy safe — but not so much that
    every line is predetermined.
20. When in doubt, make the smallest reversible move that increases information.

**This project values:** durable truth over ephemeral state · explicit contracts over hidden coupling ·
replay over hope · evidence over assertion · bounded autonomy over either micromanagement or chaos ·
simple semantics over fashionable infrastructure · accumulated learning over repeated rediscovery.

**Do not confuse a pattern with its implementation.** Many of the patterns here came from much larger
enterprise systems — Command, Observer, messaging, orchestration, adapters, receipts, retries,
correlation, durable queues. Do not copy the old machinery blindly: preserve the semantics that made
those systems reliable, and implement only the amount of machinery this system actually needs.

**When reviewing an old architectural decision**, ask *"what problem was this protecting us from?"*
before asking *"would I design it differently?"*. If the original problem still exists, preserve the
protection unless you have a demonstrably better mechanism. If runtime evidence disproves the
architecture, update the architecture — do not defend yesterday's diagram against today's facts.

**THE NORTH STAR.** Build a system whose intelligence accumulates. Every story should leave the
factory slightly more capable of building the next story. Every failure should make the next failure
cheaper. Every architecture decision worth paying for should be reusable. Every agent should inherit
more context than the previous one. The goal is not merely to produce software faster; the goal is to
create an engineering system that **remembers why it works**.

---

## 12. REFERENCES

**The three reference stories, in reading order**

1. **ARCH-HANDOFF** (this document) — architecture, boundaries, operating model.
2. **DEEP1** — the data pipeline (ODS → warehouse → screen) and the traps that cost real hours.
3. **SOP1** — factory / queue-health doctrine, for judging whether Forge is healthy.

**Repository documents**

- `docs/agent/MEMORY.md` — decision log: short facts that are expensive to rediscover.
- `docs/agent/FORGE-WORKSHOP.md` — the boot sheet / HELM manifest and how to choose an operating mode.
- `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` — the operating contract for database work.
- `docs/agent/PERSON-PROPERTY-DESIGN.md` — the person ↔ property model and its four rules.
- `docs/agent/skills/README.md` — skill inventory, with the "can run" column.
- `testv2/README.md` — the pattern statement for the glass-box tier.
- `docs/catchup-wbs-design.md`, `docs/agent/packets/PROJECTS-MVI-01.md`, `PROJECTS-MVI-02.md` — the
  Projects/MVI design and packets.
- `docs/ARCH-01-README-SUPPLEMENT.md` — the ARCH-01 supplement (still present and referenced).
- `AGENTS.md` — the repo-owned handbook (always/ask/never, error-capture obligation, delivery rule).

**Tooling that is installed and runs here:** `ripwire`, `semgrep`. Use `pnpm rw:map` /
`pnpm rw:for "<task>"` — a bare `ripwire .` maps `.next` build output and the gitignored reference
clones, because ripwire does not honor `.gitignore`.

---

## APPENDIX A — DATED VOLATILE CHECKPOINT (2026-08-24), SUPERSEDED

Kept only so a historical note can be traced. **Do not treat any hash below as current, and do not
copy this pattern** — volatile facts belong in a dated packet, per §8.

> Observed `main` HEAD before that continuity commit: `f3bb4100` (media-count casts, CORE nav, capsule
> top-nav, property-admin 2xl table breakpoint, rounded panels, public Portal link). Reference points:
> `da9d8024` (interaction-component facelift), `83228eb4` (UI Lab portal primitives), `72d72031`
> (Marlowe Gmail artifacts), `5c979659` (Attention/showings/activity, nav declutter, glass polish,
> Security rename), `b3af37fa` (the matching ARCH-01 Git checkpoint).
>
> The facelift promoted custom portal primitives only where materially useful: tables, pagination,
> search, dialogs and responsive cards. It preserved the custom Grok-built glass design and added no
> component dependency.

**Run `git log` for the current truth.**

---

## APPENDIX B — MARLOWE GMAIL CENSUS (2026-08-24): PARTIAL EVIDENCE ONLY

Artifacts: `docs/marlowe-gmail-relationship-census-report-2026-08-24.md` and
`docs/marlowe-gmail-relationship-census-private-2026-08-24.csv`.

**Partial evidence only — this is explicitly not a full census.** 7,696 messages, 2011-06-26 through
2013-12-31, 2,018 external identities; 115 two-way, 60 owner-initiated outbound-only, 1,843
inbound-only, 1,140 automated/bulk evidence. Connector results were nondeterministic and incomplete.
No message bodies, subjects, attachments or retained snippets.

The correct architecture (unchanged, and see §5.5): source metadata extraction → source-faithful
ODS/load layer → cleansing/classification → identity reconciliation → relationship mart or governed
canonical promotion.

Highest-value first pass: person/name, email, phone, iMessage/WhatsApp reachability when available,
provenance/source, first/last contact, direction/count metadata. Organization is weak enrichment;
most remaining Google metadata is noise or stale. Do not start with broad body ingestion.

**Apple Contacts:** 2,573 staged imported contacts are visible through Clients but remain
non-canonical. Preserve the `l_person` / load projection versus `person` / `person_identity` boundary.

---

*This document is the source for the `ARCH-HANDOFF` Story Board row. A later explicit decision from
Chris supersedes anything here and must be durably recorded in `docs/agent/MEMORY.md`.*











