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
>
> **Brought up to the Rust port (2026-09-28).** The 09-12 text described a TypeScript application
> (`services/`, `ui/`, `testv2/`, Next.js) that the port retired. §5 and §10 are rewritten against the
> code; §1, §3, §4, §6, §7, §9 and §12 had their stale lines replaced. The doctrine is unchanged. When
> this file and `AGENTS.md` disagree, `AGENTS.md` wins.

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
- Tests are evidence, not ritual. Full regression requires Chris's explicit authorization.
- Human judgment is reserved for architecture, security, destructive operations, visual polish, credentials, and ambiguous business semantics.
- Agents execute captured architecture; they do not casually reinvent it.
- Runtime evidence can overturn architecture; stale documentation cannot overrule fresh facts.
- Persist important learning so each failure makes the factory better.
- **The application is Rust, end to end** (§5). A screen reaches the database through route → service → repository → DAO, never directly. Zero TypeScript: even Google sign-in is Rust (`rust/server/src/api/google_auth.rs`), so any live TypeScript is unfinished work.
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
`cargo check` for the crates touched; `pnpm ui:check` whenever `rust/ui` changed (the workspace check
does not compile the wasm build — see `AGENTS.md`, 2026-09-28); a live check against DEV
(`rust/server/tests/*_dev.rs`, `scripts/rust-live-check/`) where a real database is the disputed
property; one real smoke test where runtime integration changed.

- **Full regression runs only with Chris's explicit authorization.** A nightly or pre-major-release
  occasion is a reason to *ask*; the label does not authorize the run. (There is no `next build`:
  there is no Next.js application.)
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
   push `main` → fix failures caused by the change. **A push does not deploy** (`vercel.json` sets
   `git.deploymentEnabled: false`): production is compiled on the Mac and shipped by
   `pnpm deploy:prod`, and deploying — like any command against the production database — is
   Chris's explicit call every time (`AGENTS.md`, "Never"). Verify production after he deploys. An explicit
   read-only / diagnosis-only / context-reconstruction / no-mutation / no-push instruction still
   controls. This doctrine is never permission to mutate during a read-only task.
3. **`main` is the canonical working line.** Commit and push small completed changes frequently.
   Feature branches, review branches, preview-only releases, PRs, squashes, temporary integration
   branches and stash choreography are not the default and require Chris's explicit request. Two
   exceptions are now house rules (`AGENTS.md`): `git pull --rebase` before every push, and a
   separate `git worktree` from `origin/main` when another agent is working in the same folder —
   still committing straight to `main`. A Vercel Preview may be used when Chris asks for one, but a Preview is not
   completion, and work must not remain stranded in a side-branch silo.
4. **The builder owns delivery.** Whoever changes the code owns the complete result: implementation,
   targeted tests, Git synchronization, ordinary merge/conflict resolution, commit, push to `main`,
   production verification once Chris has deployed, and immediate correction of failures it caused. Do not hand
   integration, branch cleanup, release mechanics or deployment repair back to Chris. "Done" means
   the feature is on `main` (`git log origin/main` shows it), its schema is on DEV and PROD, and —
   once Chris has deployed — production has been observed working.
5. **Fix forward.** A broken build, UI defect, type error or ordinary runtime defect is feedback,
   not a reason to create release bureaucracy. Fix it immediately and push the correction; use
   rollback only when Chris directs it or it is clearly the fastest recovery. Ordinary Git
   conflicts, failed builds and correctable defects are worker responsibilities, not
   human-decision stop conditions.
6. **Test budget.** Run the smallest targeted tests and `cargo check` that prove the changed seam. A
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

**Read this before the older-sounding parts of any other architecture note.** Between 2026-09-06 and
2026-09-11 a TypeScript service tier (`services/`), an MVI page runtime (`ui/`) and a glass-box test
tier (`testv2/`) were built. The Rust port (2026-09-21 onward) replaced all three and the Next.js host
with them; `services/` and `ui/` are deleted, and `testv2/` keeps only a README and two orphaned
helpers. What survives is their *shape*: one service kernel, one composition
root, one MVI pattern for every screen, and a fake-repository seam that proves a service without a
database. What follows is the Rust application as it is on `main`.

### 5.1 One Rust workspace

`rust/Cargo.toml` has ten crates. Dependencies point one way: UI → server → service/workflow → db.

| Crate | Owns |
| --- | --- |
| `rust/core/domain` | types and rules — no I/O, no SQL, no HTTP |
| `rust/core/db` | the DAOs, the ONE pool per process (`pool.rs`, `shared.rs`), `DbFailure` and its capture (`capture.rs`), the outbox and command-receipt tables |
| `rust/core/service` | the service kernel: `AbstractService`, `ServiceRuntime`, `ServiceContext`, authorization, audit, domain events, lifecycle, mailbox, error sink |
| `rust/core/workflow` | the transaction state machine: `WorkflowEngine`, `TxStore`, instances, tokens, tasks, timers |
| `rust/core/auth` | an empty shim (re-exports `domain`); sign-in and sessions live in `rust/server/src/api/google_auth.rs` and `ui_auth.rs` |
| `rust/server` | HTTP (axum), identity, the services, the composition root, and the website itself (`site.rs`) |
| `rust/ui` | the website and the portal: one Yew/WebAssembly app |
| `rust/integrations` | Mux, Google, Apple, BoldSign, Neon, mail, WhatsApp |
| `rust/forge` | the Forge delivery engine (§6) — tooling, not product |
| `rust/cli` | operator commands: `db-tool`, the `forge` gates, Apple intake, media backfills |

The retired TypeScript **left the tree on 2026-10-02** and is archived outside it at
`~/Documents/forge-legacy-ts-archive-2026-10-02/` — 432 files, byte-identical copy plus a zip, taken at
`5b4e5aa4` (`docs/agent/DEAD-TS-DOWNSIZE.md` §5.2, `docs/agent/BROKEN-TS-INVENTORY.md`). Read it there for intent;
it is specification to translate, **never** a module to import, repair or restore — `arch_boundary__012` fails if
anything under `legacy/` comes back. The repository itself holds no TypeScript at all.

### 5.2 Services — one kernel, one composition root

Every domain operation is a method on an `AbstractService` registered in **one** catalog:
`rust/server/src/composition.rs`, whose `registrations()` is the service map (32 services). A
service is typed over a repository trait and built over a DAO; a method runs in a fixed order —
**authorize → do the work through the repository → audit** — and a route reaches it only through
the registered mailbox (`execute_registered`), so the queue, the timeout and the refusals apply to
every door: HTTP, the engine and MQ alike. The recipe, and which services exist, is
`docs/agent/MAP-services.md`; the rule that services with real operations declare their own
descriptor instead of using `abstract_service!` is in `docs/agent/MEMORY.md` (2026-09-28). Do not
build a second catalog and do not construct a service by hand.

**Failure semantics** (the repo-wide error-capture obligation, in Rust):

- **Expected** business outcomes — validation failures, FORBIDDEN, not-found — are
  `CoreServiceError::Business` or a 4xx `ApiError`. They are audited control flow, **not** error rows.
- A database failure is a `DbFailure`, which announces itself to `app_error` when it is constructed.
  Any 5xx `ApiError` and any panic (`rust:panic`, level `fatal`) are captured too. Never swallow a
  `Result`; never `let _ =` a failure you did not decide is unreportable.

**Repository boundary rule.** A DAO turns driver values into stable domain types before they leave
`rust/core/db`. Bind parameters, never string-built SQL; a `uuid` column needs `$1::uuid`.

### 5.3 The UI — one Yew app, one `Screen` trait

The browser gets one WebAssembly app (`rust/ui`) from the Rust server (`rust/server/src/site.rs`);
there is no Next.js. Every screen implements the `Screen` trait (MVI: `Model`, `Msg`, a pure
`update`, a pure `view`); side effects leave `update` as `Cmd` values and are executed in exactly one
place, `rust/ui/src/app/exec.rs`, the only UI code allowed to touch `web_sys`. URLs live only in the
endpoint catalogue `app/api.rs`. The screen table `app/registry.rs` is the only path → screen map;
the router, both menus and the headless walk are generated from it. The browser reaches
`/api/portal/*` only (session cookie, `rust/server/src/api/portal_bridge.rs`); `/v1/*` is internal.
The contract, the holds (Marketing, WhatsApp Activation) and the recipe for a new screen are
`docs/agent/UI-SCREEN-ARCHITECTURE.md`.

### 5.4 Tests that prove it

- `#[cfg(test)]` beside the code; a service is proved against an in-memory fake of its repository
  trait (the role `testv2/` used to play) — no HTTP, no database.
- `rust/server/tests/*_dev.rs` run against a real DEV database, which is the only way the port's
  worst three bugs were ever caught. `scripts/rust-live-check/` is the live check.
- UI tests only compile with `--features wasm` (`cargo test -p ui --features wasm`); a plain
  `cargo test -p ui` compiles none of the screen tests. `pnpm ui:check` is the pre-push gate.

### 5.5 Boundaries that are settled — do not reopen without runtime evidence

1. **Screens do not read the database.** Screen → endpoint → route → service → repository → DAO.
2. **Business truth lives in the domain and its services.** The workflow crate orchestrates only;
   Forge is tooling beside the product, and the product does not depend on it.
3. **A new domain is a service in `rust/server/src/<domain>`** with a repository trait, a DAO in
   `rust/core/db`, types in `rust/core/domain`, registered in `composition.rs`. No parallel roots.
4. **Mutations are commands:** intent in, canonical service mutates, receipt/event proves it
   (`rust/server/src/command_runtime.rs`, §6.4). Relational state stays the source of truth.
5. **Presentation state belongs to the screen's own `Model`**; a screen never reads another
   screen's state, and side effects go through `Cmd`, never a hand-rolled browser call.
6. **Conditional UI is derived from available data.** No listing-specific special-casing.
7. **A fixture is never a runtime fallback.** A read failure surfaces as an explicit unavailable
   state.
8. **Media is stored in Neon.** `media` holds the original and its derived `web`, `card` and `thumb`
   copies as bytea (the "vault"); `property_media` owns role and order. The public route
   `/api/media/{id}?size=card|thumb` falls back to `web` and is cached for a year. Films go to Mux.

### 5.6 The data pipeline, and the paging standard

**The pipeline doctrine lives in DEEP1 — read it second, after this document.** Summary of the rule
that matters most: **ODS** is the `l_*` tables (raw intake, written by intake scripts, write-only,
and **nothing client-facing may ever read one**); the **warehouse** holds only what the screen
contract needs, cherry-picked out of ODS by promotion code, which is the only code permitted to
read an `l_` table; the **screen** reads the warehouse through a service, never through a
repository inline. L keeps everything, which is what makes the warehouse safe to be lossy:
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

### 5.7 The Contact / CRM spine and the macOS integration edge

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
Story Board (storyboard_story)
  -> durable agent_work_item command
  -> launchd com.culebraluxe.agent-worker, every 180s (scripts/agent-scheduler.mjs installs it)
  -> scripts/agent-worker-once.sh -> the forge-worker binary (rust/forge/src/bin/forge_worker.rs)
  -> the FORGE_SDLC workflow (rust/forge/definitions/FORGE_SDLC-v6.xml) on rust/core/workflow
  -> RoleHarness (rust/forge/src/engine/runner.rs) per role: scout, architect, lead, smith, qa, dev_ops
  -> model / tools
  -> evidence rows
  -> storyboard_story_run / forge_engine_task_execution / terminal work item
```

- **Story Board** = specification / architecture truth.
- **`agent_work_item`** = durable command queue and the single-active lock.
- **the worker** = command invoker; the engine (`rust/forge`) decides what happens next and writes
  that decision to Neon.
- **`RoleHarness`** = execution abstraction; one role, one assignment, one report.
- **`storyboard_story_run`**, **`forge_engine_task_execution`**, **`forge_tool_artifact`** = durable
  evidence / history. Status is read from these rows, never from a log.

Where to open a file is `docs/agent/MAP-engine.md`. The TypeScript harness (`agent-runtime/`) is
retired.

**One story = one command.** The factory must remain runtime-neutral above the harness boundary.
Logical model profiles must not leak provider-specific model names into canonical command semantics.

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
> evidence (the guard is `rust/forge/src/engine/execution_target.rs`). History gaps are recovered
> additively and idempotently — never by re-running work. (`pnpm forge:sync-history` was that tool;
> it is a dead TypeScript script and has no Rust port yet.) See **SOP1** for the operator doctrine,
> and DEEP1 §6 for the database-target
> trap that makes "the resolved target" a diagnostic you must not trust without printing the
> connection host.

### 6.3 Workflow engine boundary

**Application owns canonical business truth.**

The state machine (`rust/core/workflow`, formerly `workflow_engine`) owns orchestration: tokens,
transitions, timers/jobs, retries, fork/join, generic human-task mechanics, terminal semantics. One
engine step is one database transaction (`TxStore::with_tx`).

The transaction runtime (`rust/forge/src/engine/re_*`, formerly `workflow_app`) owns: the
application port (`re_port.rs`), facts (`re_facts.rs`), command routing (`re_commands.rs`),
claim-first command receipts (`re_receipt.rs`) and the verbs (`re_runtime.rs`). The API reaches it
through `rust/server/src/api/engine.rs` on a bounded worker pool, because the engine blocks. Detail:
`docs/layers/WORKFLOW.md`.

**The engine must not know CRM/domain tables.**

**Proven capabilities — do not replace the engine with a new orchestration framework unless evidence
requires it:** real Postgres persistence · concurrent joins · transactional atomicity · deterministic
command replay · idempotency · retries/failure handling · timer/lease/requeue · operational recovery
seams.

### 6.4 Business command architecture

The canonical Business Command layer exists: `rust/server/src/command_runtime.rs` (envelope,
dispatcher, receipts through `CommandReceiptDao`, domain events through `DomainEventOutboxDao`), on the
kernel types in `rust/core/service`. Extend it; do not build a second one. Its shape:

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
concepts. The outbox is the `outbox_message` table, written and delivered by `rust/core/db/src/outbox.rs`
(`rust/server/tests/mq_runtime_dev.rs` and `service_atomicity_dev.rs` prove it against DEV).

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
release gates, the analyzer-tool boundary, and the deploy receipts — read **SOP1**, then the command
table in `docs/agent/MAP-engine.md`, rather than this section.

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
    - during read-only/diagnosis work: push. Never deploy or touch the production database
      without Chris's explicit go. (During authorized implementation, delivery to main is implied.)
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
  RUN SCOPED TESTS ONLY. Full regression requires Chris's explicit authorization, including at a
  promotion boundary.
    - targeted tests for the changed seam
    - adjacent tests where justified
    - cargo check for the crates touched; pnpm ui:check if rust/ui changed
    - pnpm build if routing/server/UI/deployment surface changed
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
    6. cargo check (and pnpm ui:check for rust/ui) passes
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
  7. For authorized implementation, commit to main and push (git pull --rebase first). Deploying is
     Chris's call; verify production once he has deployed.
  8. Update durable evidence.
  9. STOP. Do not continue into adjacent backlog stories.

FINAL REPORT
  Story implemented; architecture actually used; files changed; schema/migrations changed; canonical
  services reused; new contracts/interfaces/classes; invariants proved; exact scoped tests run and
  results; cargo check/build result if applicable; runtime/smoke evidence if applicable; environment
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
- Catch-Up → `/portal/catch-up`
- Contracts → `/portal/deals`
- Cabinet → `/portal/documents`
- Projects → `/portal/projects`, Workflows → `/portal/workflows`, Forms → `/portal/forms`, Seller
  Strategy → `/portal/core/seller-strategy` (the authoritative list is `rust/ui/src/app/registry.rs`)

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
> plane for status; query the code and schema for architecture claims. The 2026-09-11/12 snapshot it
> replaces (four MVI-wired TypeScript screens, `contract` unreleased, zero PROD projects) is history.

**As of 2026-09-28** (each line names the check that proves it):
> **Superseded 2026-10-02 — the TypeScript and menu lines below are history, not status.** Measured that day: the
> repository holds **no TypeScript at all** (the retired estate was archived out of the tree — §5.1 above,
> `docs/agent/DEAD-TS-DOWNSIZE.md` §5.2); `forge ts-sweep` scans **3 files, 0 cannot load**;
> `pnpm typecheck` is down to a single `TS18003` — nothing left to compile; and the menu has **0 dead commands**
> (104 scripts, and the 12 that name a file all resolve). The 2026-09-28 numbers below stand as that day's record.
> So do the dead-command rows that follow, whose failure mode is fixed by construction: a command whose target is
> gone is now caught by the same check this line reports. The one token in the menu that does not resolve is
> `start` → `rust/target/release/http` — an unbuilt release artifact, not a missing source file.



- **UI.** `rust/ui/src/app/registry.rs` has 58 entries: 56 on the `Screen` trait (Marketing became the
  native Publishing Center on 2026-09-28, and the old global loop was then deleted), 2 external
  (WhatsApp Activation, `/portal`). `docs/agent/UI-SCREEN-ARCHITECTURE.md` STATUS.
- **TypeScript.** No Next.js application; Google sign-in is Rust. `pnpm broken:ts:sweep`: 251 files
  scanned under `scripts/` + `agent-runtime/`, 187 marked broken on purpose, tree and inventory
  agree. **51 tracked `.ts` files outside `legacy/` carry no banner** (agent-runtime 29, scripts 16,
  `workflow_engine/lib/workflow` 4, `testv2/engine_tests` 2) and some are still wired to `pnpm`
  (`smoke:prod` → `scripts/prod-smoke.ts`, `forge:silent-failure-gate`); they are unfinished port
  work, not exceptions.
- **Dead commands still on the menu.** `forge:sync-history`, `forge:tools`, `forge:decision` and
  `story:status` point at bannered scripts and cannot run. `test:engine` (and so `pnpm test`) runs
  `testv2/engine_tests/*.test.ts`, which matches no file.
- **The architecture hard gate never runs.** `rust/forge/src/engine/qa_adjudicate.rs` reads
  `arch_ran` but no Rust code sets it, so the gate reads **INCOMPLETE**, never PASS. knip and
  dependency-cruiser are now installed but check TypeScript, which is not the product; the Rust
  boundaries are held by crate dependencies and the compiler.
- **Release evidence** is now derived in Rust (`derive_release_evidence`,
  `rust/forge/src/engine/role_slice.rs:93`). Whether real runs populate it (the old TECH-DEBT-07) is
  **not verified** — ask the rows.
- **Deploys** are manual: `pnpm deploy:prod` compiles on the Mac; git pushes do not deploy.
- **Media.** Listing cards use the `card` copy (migration 252, DEV and PROD, backfilled 2026-09-28).
- **Data pipeline.** See DEEP1 and `docs/agent/MEMORY.md` (2026-09-27/28): the warehouse promotion
  was decided PORT, and Apple lands in its own tables and is reconciled, never written over `person`.

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

- `AGENTS.md` — the house rules; they outrank everything else here.
- `docs/agent/ORIENTATION.md` — the map: layers, commands, where to look for X.
- `docs/agent/MEMORY.md` — decision log: short facts that are expensive to rediscover.
- `docs/agent/MAP-services.md` — the service recipe and registration.
- `docs/agent/UI-SCREEN-ARCHITECTURE.md` — the `Screen` contract, holds and the recipe for a screen.
- `docs/agent/MAP-engine.md` — the Forge engine, its commands and its rows.
- `docs/layers/SERVICES.md`, `DB.md`, `WORKFLOW.md`, `FORGE.md`, `UI.md` — one page per layer.
- `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md`, `docs/agent/SOP-DEV-REFRESH.md` — database work.
- `docs/agent/DEV-OPS-RELEASE.md` — build, deploy, release record, smoke.
- `docs/agent/PERSON-PROPERTY-DESIGN.md` — the person ↔ property model and its four rules.
- `docs/agent/BROKEN-TS-INVENTORY.md` — what TypeScript is dead, and its Rust home.
- `docs/agent/HANDOFF-TEMPLATE.md` — the shape a stopping session leaves behind.
- `docs/rust-contributing.md` — making a Rust change without the known traps.

**Tooling that is installed and runs here:** `ripwire`, `semgrep`. Use `pnpm rw:map` /
`pnpm rw:for "<task>"` — a bare `ripwire .` also maps the gitignored reference clones, because
ripwire does not honor `.gitignore`.

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
