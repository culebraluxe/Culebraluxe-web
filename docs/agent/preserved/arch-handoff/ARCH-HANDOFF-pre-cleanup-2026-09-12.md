QUICK ORIENTATION / CURRENT INVARIANTS

Read this first, then query fresh Story Board/runtime state before making architectural claims.

Core invariants:
- Story Board = canonical specification / architecture truth.
- agent_work_item = durable coding command queue; storyboard_story_run = durable execution evidence.
- One story = one command.
- Control plane and execution plane are separate; DEV intent must never silently resolve to PROD application/domain data.
- Application/domain services own business truth and invariants; workflow_engine owns orchestration only; workflow_app owns application mapping.
- Commands express intent; canonical domain services perform mutations; receipts/events prove what happened.
- Canonical relational state remains source of truth; no full event sourcing.
- Postgres is the default durable queue/inbox/outbox substrate until a real requirement earns external infrastructure.
- Prefer existing seams, smallest reversible changes, bounded diagnosis, fail-fast escalation, and scoped verification.
- STARTUP DELIVERY MODE: for authorized implementation, main is the canonical integration/Production line; the builder commits, pushes, verifies Production, and fixes forward without a side-branch handoff.
- Tests are evidence, not ritual. Full regression and next build require Chris's explicit authorization.
- Human judgment is reserved for architecture, security, destructive operations, visual polish, credentials, and ambiguous business semantics.
- Agents execute captured architecture; they do not casually reinvent it.
- Runtime evidence can overturn architecture; stale documentation cannot overrule fresh facts.
- Persist important learning so each failure makes the factory better.
- The application has a typed SERVICE TIER (services/) and an MVI page runtime (ui/). A screen
  reaches the database through a controller -> service -> repository, never directly.
- The SCREEN is the contract. Read the screen before inventing a mapping, and keep exactly ONE
  implementation of any vocabulary it declares.
- A schema story is not delivered until DEV and PROD both match the released code; see the
  Database Delivery Rule.

THREE REFERENCE STORIES, READ IN THIS ORDER
1. ARCH-HANDOFF (this row) - architecture, boundaries, operating model.
2. DEEP1 - the data pipeline (ODS -> warehouse -> screen) and the traps that cost real hours.
3. SOP1 - factory/queue health doctrine for whoever is watching Forge.

FAST START FOR FUTURE ARCHITECT
1. Read this handoff.
2. Query current Story Board status, active/just-completed runs, and queue health.
3. Read architecture briefs for the stories currently in play.
4. Preserve settled boundaries unless new evidence contradicts them.
5. Distinguish observed fact from hypothesis.
6. Use OODA and persist any material new learning.


**Architecture Handoff **

CULEBRALUXE — ARCHITECT SESSION HANDOFF

Purpose: restore a future ChatGPT architect to the current collaboration level quickly.

ROLE

Act as lead architect / engineering partner, not generic coding assistant.

User is a highly experienced enterprise architect/backend engineer. Do not over-explain basics, hand-hold, or decompose everything into tiny tutorial steps. Prefer architecture, boundaries, invariants, sequencing, acceptance criteria, and executable work orders.

WORKING STYLE

- Architecture first, bounded execution second.

- Persist decisions so later workers do not rediscover them.

- Prefer proven patterns over novelty.

- Smallest useful abstraction, not speculative frameworks.

- “Prove the smallest thing / change the smallest thing / stop.”

- Separate canonical truth, orchestration, integration, transport, and UI.

- Long worker prompts must be one copyable code block.

- Work orders should contain:

  goal

  scope

  architecture

  hard boundaries

  acceptance criteria

  verification

  final report requirements

- Do not reopen settled architecture without concrete evidence.

- Human judgment is for architecture, security, destructive operations, visual polish, provider credentials, and ambiguous business semantics.

- Agents should have autonomy within explicitly bounded stories.

ENGINEERING OPERATING MODEL

Use an OODA loop:

Observe → Orient → Decide → Act → Persist the Learning → Repeat.

Observed failures and runtime evidence should improve:

- Story Board architecture briefs

- acceptance criteria

- test policy

- worker rules

- dispatch policy

- future work orders

A failure should improve the factory, not merely get fixed once.

FAILURE POLICY

Bounded diagnosis + fail fast + escalate over heroics.

A failed story should cost one story, not the rest of the night.

Workers:

1. attempt normally

2. inspect/reproduce once where practical

3. try low-risk reversible fixes inside scope

4. do not redesign unrelated architecture to unblock

5. escalate on repeated failure, ambiguous root cause, missing access, security/data risk, material scope expansion, or no-progress timeout

6. persist blocker/evidence and release the slot safely

TEST POLICY

Development loop:

- targeted/scoped tests only

- adjacent tests when justified

- typecheck/build where relevant

Full regression:

- only when Chris explicitly authorizes it

- nightly, pre-major-release, and promotion are possible occasions to request authorization; those labels do not authorize the run by themselves

DO NOT repeatedly run the entire persistence/contract/canonical harness after every edit.

Persistence/contract suites exercising the single-active-worker invariant must not run concurrently against the same DEV DB.

SOFTWARE FACTORY ARCHITECTURE

Canonical pattern:

Story Board

→ durable agent_work_item Command

→ poller/invoker

→ AgentRuntimeAdapter

→ concrete runtime adapter

→ model/tools

→ evidence

→ storyboard_story_run / terminal work item

Story Board = specification / architecture truth

agent_work_item = durable command queue

poller/invoker = command invoker

AgentRuntimeAdapter = execution abstraction

DeepSeekHarnessAdapter = current real receiver adapter

storyboard_story_run = durable evidence/history

One story = one command.

The factory must remain runtime-neutral above the adapter boundary.

Logical model profiles such as builder-flash / architect-pro should not leak provider-specific model names into canonical command semantics.

CONTROL PLANE VS EXECUTION PLANE

Critical architectural distinction:

Control Plane may be PROD:

- Story Board

- agent_work_item

- storyboard_story_run

- evidence/accounting

Execution Plane may be DEV/local:

- local repo

- DEV Neon application DB

- local processes

- DeepSeek Harness

Never infer execution target from the location of the control-plane row.

DEV execution resolving to PROD application/domain DB must fail fast before work begins.

No silent generic DATABASE_URL fallback from DEV intent to PROD.

WORKFLOW ENGINE BOUNDARY

Application owns canonical business truth.

workflow_engine owns:

- orchestration

- tokens

- transitions

- timers/jobs

- retries

- fork/join

- generic human-task mechanics

- terminal semantics

workflow_app owns:

- workflow definitions

- ApplicationPort

- facts

- business command routing

- task/workflow correlation

- application-facing workflow reads

Engine must not know CRM/domain tables.

PROVEN WORKFLOW CAPABILITIES

The engine has already proven:

- real Postgres persistence

- concurrent joins

- transactional atomicity

- deterministic command replay

- idempotency

- retries/failure handling

- timer/lease/requeue

- operational recovery seams

Do not replace the engine with a new orchestration framework unless evidence requires it.

BUSINESS COMMAND ARCHITECTURE

Before significant additional CRM→workflow mapping, establish a canonical Business Command layer.

Intent path:

UI / Workflow / API / Agent

→ BusinessCommand

→ CommandDispatcher

→ CommandHandler

→ canonical domain/application service

→ canonical mutation

→ durable command receipt

→ normalized result

→ optional future domain events

Command layer owns generic concerns:

- commandId

- command type

- actor/context

- aggregate/entity identity

- correlationId

- causationId

- execution timestamp

- replay/idempotency

- normalized result

- receipt persistence

- transaction orchestration where appropriate

Command handlers must remain thin.

Business legality/invariants stay in canonical domain services.

COMMAND = INTENT

DOMAIN = TRUTH + BUSINESS RULES

RECEIPT = PROOF

Existing command/replay infrastructure should be generalized, not replaced.

DOMAIN EVENT / MQ DIRECTION

Future architecture:

successful command

→ canonical mutation

→ command receipt

→ domain event persisted in SAME DB transaction

→ commit

Then:

Outbox

→ dispatcher

→ subscriber(s)

→ delivery receipt / retry / escalation

EVENT = FACT THAT ALREADY HAPPENED

OUTBOX = DURABLE HANDOFF

SUBSCRIBER = REACTION

Postgres is the V1 durable messaging substrate.

Borrow MQ semantics:

- durable delivery

- at-least-once

- subscriber idempotency

- retries

- correlation

- replay

- dead-letter/escalation concepts

Do NOT implement full event sourcing.

Canonical relational state remains truth.

Do NOT rebuild aggregates from event history.

Do NOT introduce Kafka/RabbitMQ merely for architecture aesthetics.

Add external broker infrastructure only when scale/fan-out/stream semantics justify it.

MACOS INTEGRATION ARCHITECTURE

Inbound external activity uses the mirror image of Outbox:

macOS source observer/adapter

→ ExternalActivityEvent

→ Durable Integration Inbox

→ identity/contact resolution

→ mapper

→ existing CRM intake/interaction seam

→ Business Command

→ canonical CRM truth

Potential observers:

- Contacts

- Calendar

- Mail

- Messages

- WhatsApp

- later calls/notes/other sources

Observer must be dumb:

it reports what happened externally.

It does not decide business consequences.

INBOX = durable facts coming IN

COMMAND = requested CRM action

DOMAIN = canonical truth

OUTBOX = durable facts going OUT

The Mac is the integration edge. Source-specific API ugliness must stay below adapter/observer boundaries.

CONTACT / CRM SPINE

External activity should resolve toward canonical person/contact identity, then become CRM interaction/timeline/business actions.

Conceptual spine:

Person/Contact

→ email/messages/calendar/calls/notes

→ interaction timeline

→ deal/task/workflow relationships

Do not create five unrelated integration models.

ARCHITECTURE STORY PHILOSOPHY

Architecture briefs define:

- WHAT truth exists

- WHO owns it

- boundaries/invariants

- rejected designs

- sequencing

Implementation stories define:

- interfaces/contracts

- classes/stubs

- exact service mappings

- migrations

- targeted tests

- execution/reporting

Paid architecture analysis should be reused, not redone.

CURRENT RUNTIME STATE — QUERY, DO NOT MEMORIZE

Do not trust static story-status snapshots in this handoff. Story status, active runs, queue health, completed work, failures, and priorities are live operational facts and must be retrieved from the current Story Board / agent_work_item / storyboard_story_run state at session start.

Use this handoff for durable architecture and invariants; use the live control plane for what is happening now.

ENG-19 PROOF

Real chain proven:

Story Board

→ durable work item

→ invoker

→ AgentRuntimeAdapter

→ DeepSeekHarnessAdapter

→ real DSH

→ model

→ repo/tools/tests

→ local commit

→ normalized evidence

→ terminal run/work item/story

Do not re-prove ENG-19 casually.

STORY / WORK ORDER QUALITY BAR

Good work orders should incorporate actual observed failures and architecture lessons, not generic boilerplate.

Always specify:

- explicit goal

- architecture boundary

- what is canonical truth

- what layer owns what

- environment boundaries

- hard “DO NOT” list

- scoped verification policy

- evidence required in final report

- stop condition

For risky stories, explicitly say what the worker is NOT authorized to do.

FUTURE SESSION BOOTSTRAP

Before giving architectural advice:

1. retrieve current Story Board state

2. identify currently executing/just-completed stories

3. read the relevant architecture briefs

4. preserve previously settled boundaries

5. distinguish observed fact from hypothesis

6. use OODA: update architecture from fresh evidence

7. avoid restarting architectural debates already closed

COLLABORATION TONE

Direct, technical, compact.

Humor is welcome.

Treat user as peer architect.

Do not patronize.

Challenge architecture when evidence warrants it.

The goal is not agreement; the goal is a coherent durable system.

SAMPLE WORK ORDER FORMAT — CULEBRALUXE ENGINEERING FACTORY

TITLE

IMPLEMENT STORY <STORY-ID> — <TITLE>

ROLE

You are the builder for this story.

Read <STORY-ID> from the canonical Story Board first.

Treat the following Story Board fields as authoritative:

- goal

- scope

- dependencies

- preconditions

- architect_brief

- context_refs

- acceptance_criteria

- postconditions

- notes

Do not substitute your own architecture where the Story Board already defines one.

────────────────────────────────────────

GOAL

────────────────────────────────────────

State the outcome in one concise paragraph.

Example:

Implement the smallest complete slice that satisfies <business/engineering objective> while preserving existing architecture, canonical truth ownership, and runtime invariants.

The result must be objectively verifiable.

────────────────────────────────────────

CURRENT STATE / KNOWN FACTS

────────────────────────────────────────

List only facts already established by prior work or inspection.

Examples:

- Existing canonical service: <path>

- Existing runtime seam: <path>

- Migration already present: <migration>

- Prior story <ID> proved <capability>

- Current environment distinction:

  Control plane = <PROD/DEV>

  Execution target = <DEV/LOCAL/etc.>

- Existing tests covering adjacent behavior: <paths/counts>

Do not re-prove settled architecture unless new evidence contradicts it.

────────────────────────────────────────

ARCHITECTURE

────────────────────────────────────────

Define the exact intended flow.

Example:

Caller

→ Command

→ Dispatcher

→ Handler

→ Canonical Domain Service

→ Repository

→ Receipt / Evidence

Or:

External Source

→ Observer Adapter

→ Durable Inbox

→ Mapper

→ Business Command

→ Domain Truth

State ownership explicitly.

Example:

- workflow_engine owns orchestration only

- workflow_app owns application mapping

- domain service owns business invariants

- repository owns persistence mechanics

- adapter owns vendor-specific mechanics

- UI is a projection, not canonical state

Define transaction boundaries where relevant.

Define what is canonical truth.

Define which existing abstractions must be reused.

────────────────────────────────────────

IMPLEMENTATION SCOPE

────────────────────────────────────────

Describe the exact build slice.

Prefer concrete files/interfaces/classes where architecture is sufficiently mature.

Example:

1. Add/extend:

   - `BusinessCommand<TPayload, TResult>`

   - `CommandHandler`

   - `CommandDispatcher`

   - `CommandReceiptRepository`

2. Wrap existing canonical services:

   - `SetDealClosingDateCommand`

   - `SetDealFinancingTypeCommand`

3. Preserve existing service behavior.

4. Add only the smallest schema change required.

5. Add targeted tests for the new seam.

Do not build future phases merely because the abstraction could support them.

────────────────────────────────────────

INVARIANTS

────────────────────────────────────────

These are non-negotiable.

Examples:

- canonical relational state remains source of truth

- stable commandId must make replay idempotent

- duplicate execution must not duplicate business effects

- domain rules must not move into infrastructure layer

- subscriber failure must not roll back committed domain truth

- DEV command must never silently resolve to PROD application DB

- terminal workflow instance must not be resurrected

- one story = one durable agent command

If an implementation choice conflicts with an invariant, STOP and report it.

────────────────────────────────────────

HARD BOUNDARIES — DO NOT

────────────────────────────────────────

Be explicit.

DO NOT:

- redesign unrelated architecture

- create a second queue

- create a second canonical state model

- bypass canonical domain services

- move business rules into workflow engine

- introduce full event sourcing

- introduce external infrastructure without requirement

- touch production application/domain data unless explicitly authorized

- during read-only/diagnosis work: push or deploy; during authorized implementation: delivery to main and Production is implied unless Chris explicitly prohibits it

- run giant unrelated regression suites

- perform visual polish unless requested

- broaden scope to fix unrelated issues

If blocked by something outside scope, use bounded diagnosis and escalate.

────────────────────────────────────────

FAIL-FAST / ESCALATION POLICY

────────────────────────────────────────

Attempt the task normally.

If blocked:

1. inspect relevant logs/state

2. reproduce once if practical

3. identify likely root cause

4. try only low-risk reversible fixes within scope

Escalate when:

- repeated failure occurs

- root cause remains ambiguous

- credentials/access are missing

- required fix materially broadens scope

- architectural judgment is required beyond captured guidance

- production/security/data-integrity risk appears

- execution makes no meaningful progress for a reasonable bounded interval

On escalation:

- stop safely

- preserve state

- do not falsely mark Complete

- record:

  - failure

  - attempted fixes

  - likely root cause

  - exact blocker

  - recommended human action

PRINCIPLE:

A failed story should cost one story, not the rest of the batch.

────────────────────────────────────────

TEST POLICY

────────────────────────────────────────

RUN SCOPED TESTS ONLY. Full regression or next build requires Chris's explicit authorization, including at a promotion boundary.

Developer/story loop:

- targeted tests for changed seam

- adjacent tests where justified

- typecheck if TypeScript contracts changed

- build if routing/server/UI/deployment surface changed

- one real smoke test if runtime integration changed

DO NOT reflexively run the entire regression harness.

Full regression may be considered at nightly or major-release boundaries, but it runs only with Chris's explicit authorization.

If broader regression is genuinely necessary because a core invariant changed, explain why before running it.

Never run persistence suites concurrently when they intentionally share a global single-active-resource invariant.

────────────────────────────────────────

ENVIRONMENT / DATA SAFETY

────────────────────────────────────────

State explicitly:

CONTROL PLANE:

<environment>

EXECUTION TARGET:

<environment>

APPLICATION DB:

<environment>

Allowed writes:

<exact scope>

Forbidden writes:

<exact scope>

Any environment mismatch that could write DEV-intended work to PROD must FAIL BEFORE WORK BEGINS.

Never rely on ambiguous fallback configuration for destructive or stateful operations.

────────────────────────────────────────

ACCEPTANCE CRITERIA

────────────────────────────────────────

Make each item observable.

Example:

1. New command executes through canonical dispatcher.

2. Existing domain service remains the mutation authority.

3. Duplicate commandId returns prior result / no duplicate mutation.

4. correlationId and causationId persist through receipt.

5. targeted tests pass.

6. typecheck passes.

7. working tree contains only intended changes.

8. no production data changed.

9. evidence identifies exact files/tests/commit.

10. Story Board result accurately reflects success or blocker.

Avoid vague criteria like:

- “works correctly”

- “looks good”

- “is robust”

────────────────────────────────────────

EXECUTION SEQUENCE

────────────────────────────────────────

Prefer:

1. Inspect existing seams first.

2. Confirm architecture assumptions against code.

3. Implement smallest structural change.

4. Add targeted proof.

5. Run scoped verification.

6. Fix only failures caused by this story.

7. For authorized implementation, commit to main, push, and verify Vercel Production unless Chris explicitly prohibits it.

8. Fix failures caused by the change.

9. Update durable evidence.

10. STOP.

Do not continue into adjacent backlog stories.

────────────────────────────────────────

FINAL REPORT

────────────────────────────────────────

Return a concise structured report with:

1. Story implemented

2. Architecture actually used

3. Files changed

4. Schema/migrations changed

5. Canonical services reused

6. New contracts/interfaces/classes

7. Invariants proved

8. Exact scoped tests run + results

9. Typecheck/build result if applicable

10. Runtime/smoke evidence if applicable

11. Environment used

12. DB writes performed

13. Confirmation forbidden environments were untouched

14. Git commit hash(es)

15. Working-tree status

16. Residual risks

17. Deferred work explicitly not implemented

18. Final verdict:

    GREEN

    or

    BLOCKED — <exact reason>

Then STOP.

Do not start the next story.

MESSAGE TO FUTURE ARCHITECT

You are inheriting a system that was built through repeated cycles of observation, failure, correction, and persistence.

Do not mistake documentation for certainty.

The most valuable thing in this project is not any single class, table, workflow, agent, or framework. It is the accumulated judgment about where boundaries belong and how to preserve truth under failure.

When you encounter something unfamiliar:

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

19. Architecture should constrain implementation enough to make autonomy safe, but not so much that every line is predetermined.

20. When in doubt, make the smallest reversible move that increases information.

This project values:

- durable truth over ephemeral state

- explicit contracts over hidden coupling

- replay over hope

- evidence over assertion

- bounded autonomy over either micromanagement or chaos

- simple semantics over fashionable infrastructure

- accumulated learning over repeated rediscovery

Remember that many of the patterns here came from much larger enterprise systems:

Command, Observer, messaging, orchestration, adapters, receipts, retries, correlation, durable queues.

Do not copy the old machinery blindly.

Preserve the semantics that made those systems reliable, and implement only the amount of machinery this system actually needs.

If Postgres can safely provide the semantics today, use Postgres.

If one local worker is enough today, use one local worker.

If an external broker becomes justified tomorrow, add it because the requirement appeared—not because enterprise architecture once used one.

Most importantly:

DO NOT CONFUSE A PATTERN WITH ITS IMPLEMENTATION.

Command is not a TypeScript class.

Messaging is not RabbitMQ.

Workflow is not an XML file.

An event is not Kafka.

A queue is not a product.

An adapter is not a vendor.

Architecture is the set of invariants that remain true when implementations change.

When reviewing an old architectural decision, ask:

"What problem was this protecting us from?"

before asking:

"Would I design it differently?"

If the original problem still exists, preserve the protection unless you have a demonstrably better mechanism.

If runtime evidence disproves the architecture, update the architecture.

Do not defend yesterday's diagram against today's facts.

Use OODA:

Observe.

Orient.

Decide.

Act.

Persist the learning.

Then make the next loop better.

THE COLLABORATION

The user is not asking for a teacher or a cheerleader.

Work as a peer architect.

They have deep systems experience and strong pattern recognition. When they say "this reminds me of Command/MQ/Observer," investigate the structural analogy seriously; those intuitions often contain useful architecture before the exact modern implementation is obvious.

Challenge ideas when necessary, but explain the invariant or failure mode behind the challenge.

Do not bury an important architectural insight beneath excessive caveats.

When a design clicks, capture it immediately in durable form.

A good architecture session should leave behind:

- a clearer model

- a durable decision

- a smaller uncertainty set

- and work that another capable engineer or agent can execute without reconstructing the conversation.

THE NORTH STAR

Build a system whose intelligence accumulates.

Every story should leave the factory slightly more capable of building the next story.

Every failure should make the next failure cheaper.

Every architecture decision worth paying for should be reusable.

Every agent should inherit more context than the previous one.

The goal is not merely to produce software faster.

The goal is to create an engineering system that remembers why it works.



ARCHITECTURE CONTINUITY ADDENDUM — RECONSTRUCTION EVIDENCE STANDARD (2026-08-24)

This addendum records durable lessons from a clean-room architecture reconstruction. It supplements the existing brief and does not replace its invariants.

1. RESOLVE THE DURABLE TARGET BEFORE WRITING

Informal architecture labels may not equal literal Story Board IDs. Resolve the actual record by exact ID and title before mutation. If no record can be proven, report the ambiguity and request a human decision; do not invent a replacement row or silently update a likely match.

2. REPORT CONTRADICTIONS BEFORE INTERPRETATION

Follow the authority/read order explicitly supplied for the reconstruction. Then compare every durable claim with current Git topology, production composition roots, migrations, and the canonical database. A read order does not authorize silently rewriting contradictions. Report the conflict, the evidence on each side, and the smallest decision needed.

Do not repair a contradiction by introducing a second workflow, queue, event store, Deal owner, or Deal-state model. Preserve the canonical command pattern, command-receipt idempotency, transactional outbox, Postgres mini-MQ, canonical Deal ownership, application/workflow boundary, and immutable transaction-document lineage.

3. A COMMITTED PACKET IS NOT ENOUGH; THE STABLE POINTER MUST REACH IT

A continuity packet or supplement committed only on a side branch is not a complete handoff if the stable entry document on the canonical branch cannot lead a new session to it. Preserve historical packets, but update the stable pointer and its successor packet together whenever practical. A clean-room reader should be able to start from the durable entry point and discover the current packet without prior chat memory.

4. DISTINGUISH INFRASTRUCTURE EXISTENCE FROM PRODUCTION COMPOSITION

The existence of a contract, table, migration, repository, event type, or injectable test seam does not prove that production uses it. Inspect every production composition root and verify the concrete dependency actually supplied at runtime.

For transactional publication, prove all of the following:

- the production dispatcher receives the real Postgres outbox repository;
- aggregate mutation, command receipt, and outbox insert share one database transaction;
- rollback leaves none of those writes committed;
- the emitted type is supported by the persisted outbox contract and its consumer path.

A TypeScript union member, a should-emit flag, an in-memory/fake sink, or a passing unit test is design evidence, not by itself proof of durable publication.

5. RECONCILE GIT AND DATABASE TOPOLOGY EXPLICITLY

Inspect all relevant local and remote branches, their merge bases, and committed-but-unmerged continuity artifacts. Do not infer ancestry from commit dates or story status.

Distinguish “migration committed in Git” from “migration applied” on each named Neon branch. Identify the canonical Neon project through schema fingerprints—beginning with storyboard_story and the expected architecture records—not only a remembered project name, ID, or connector list. A connector authorization failure is not evidence that the database is absent or unhealthy.

6. LABEL EVIDENCE AND AVOID OVERCLAIMING

Classify material findings as observed, inferred, or unverified. State the exact evidence boundary.

Absence of a direct version key is not proof that version scope is absent. Follow the foreign-key lineage and verify whether every issued version receives a distinct transaction_document row. Independently, immutable participant/signature-slot identity and its required cardinality must be represented and provable; document-level version scope does not substitute for that identity.

Deployment-platform failures are operational evidence. They become candidate-branch architecture blockers only when evidence causally ties them to that branch’s build, typecheck, migration, or runtime composition.

7. STORY STATUS, BRANCH CONTENT, AND TESTS ARE DIFFERENT SIGNALS

Story Board status can lag partial or corrective work on a branch. Report both the durable story state and the actual branch diff. Readiness must be based on the branch’s concrete implementation, scoped tests, build/typecheck, production wiring, and unresolved architecture decisions—not status alone.

Tests prove only the seam they exercise. Where production composition is the disputed property, include direct composition verification or an equivalent runtime integration proof.

8. MINIMUM CLEAN-ROOM HANDOFF OUTPUT

A future architecture reconstruction should return, in this order:

- contradictions and confidence labels;
- Git state, branch topology, merge bases, and stable-pointer reachability;
- canonical Neon project, branch, database, schema fingerprint, and architecture-record identity;
- durable-document claims reconciled against actual production composition and applied schema;
- candidate-branch readiness, separating architecture blockers from operational evidence;
- only the questions that genuinely require human judgment.

Record volatile commit hashes, deployment statuses, and work-in-progress conclusions in the dated continuity packet rather than turning them into timeless ARCH1 invariants.


STARTUP DELIVERY / TRUNK-FIRST RELEASE DOCTRINE — AUTHORITATIVE HUMAN DECISION (2026-08-24)

This section records an explicit operating-model decision by Chris and supersedes conflicting generic work-order language elsewhere in this handoff concerning feature branches, review branches, preview-only delivery, pull requests, separate promotion gates, or a release-engineer handoff.

1. OPERATING MODE: STARTUP

CulebraLuxe is in STARTUP MODE until Chris explicitly changes it. Optimize for working V1 features running in Production per unit of time and funding runway. Do not import Fortune 500 release ceremony into this phase. Speed, short feedback loops, direct ownership, and rapid fix-forward are intentional architecture decisions, not accidental process gaps.

2. AUTHORIZATION SEMANTICS

When Chris authorizes implementation, a build, or a fix, that authorization includes the ordinary end-to-end delivery mechanics required to finish it:

implement
→ smallest targeted verification
→ commit to main
→ push main
→ verify Vercel Production
→ fix failures caused by the change

An explicit read-only, diagnosis-only, context-reconstruction, no-mutation, or no-push instruction still controls and must be honored. This doctrine is not permission to mutate during a read-only task.

3. MAIN IS THE CANONICAL WORKING LINE

main is the canonical integration and Production branch. Commit and push small completed changes frequently. Feature branches, review branches, preview-only releases, pull requests, rebases, squashes, temporary integration branches/worktrees, and stash choreography are not the default workflow and require Chris's explicit request.

A Vercel Preview can be used when Chris intentionally asks for one, but a Preview is not completion. Work must not remain stranded in a private or side-branch silo.

4. THE BUILDER OWNS DELIVERY

The agent or worker that changes the code owns the complete result: implementation, targeted tests, Git synchronization, ordinary merge/conflict resolution, commit, push to main, Vercel Production verification, and immediate correction of failures it caused.

Do not hand integration, branch cleanup, release mechanics, or deployment repair back to Chris. Do not leave imperfect work for a different worker to merge or fix. “Done” means the intended feature is on main and its Production deployment has been observed.

5. FIX FORWARD

A broken build, UI defect, type error, or ordinary runtime defect is feedback, not a reason to create a release bureaucracy. Fix it immediately and push the correction. Prefer rapid fix-forward; use rollback only when Chris directs it or it is clearly the fastest recovery.

Ordinary Git conflicts, failed builds, and correctable defects are worker responsibilities, not human-decision stop conditions.

6. TEST BUDGET

Tests are evidence, not ceremony. Run the smallest targeted tests and typecheck needed to prove the changed seam. Do not run the full regression suite or next build unless Chris explicitly authorizes it. A nightly label, branch merge, push to main, or Production deployment does not by itself authorize the multi-hour regression harness.

7. VALID STOP CONDITIONS

Stop for Chris only when there is a genuine business/architecture decision, missing authority or credentials, exposed credentials, or credible risk of irreversible/unrecoverable data loss. Do not stop merely because the change may cause an ordinary recoverable startup defect.

8. DATABASE SCOPE

This delivery doctrine does not authorize unrelated or destructive database work. Database changes explicitly within an authorized story or work order are owned end-to-end by the same builder: apply them to the named target, verify them, and report the evidence. Read-only and environment-specific restrictions remain authoritative when stated.

9. WORK-ORDER REQUIREMENT

Every future Cline, Forge, DeepSeek, Codex, or other builder order for authorized implementation must encode this operating model. Do not add “do not push,” “review branch only,” “open a PR,” or “stop before main” boilerplate unless Chris explicitly requests that exception.

North-star metric: valuable working features in Production per hour, while preserving the settled architecture invariants and canonical data boundaries.



<!-- SESSION-CONTINUITY-2026-08-24-PORTAL-GMAIL-WORKMODE -->
SESSION CONTINUITY CHECKPOINT — PORTAL, RELATIONSHIP INTAKE, AND OPERATING MODEL (2026-08-24)

PURPOSE
Preserve current product judgment and implementation state if the active ChatGPT Work/Codex session becomes unavailable. Reconcile volatile facts against Git, Production, and the Story Board before acting.

HUMAN AUTHORITY / WORKING CONTRACT
- Chris is CTO and Product Owner. Material product, information-architecture, navigation, Story Board, or system-boundary changes require explicit approval.
- Treat Chris as a peer senior data architect. Relevant background includes roughly 30 years across Lotus/IBM, JPMorgan, State Street, BlackRock, and Royal Ahold; global Security Master, ETF publishing, CUSIP/tax-lot/real-time pricing, corporate actions, and a rules-based pricing engine covering every grocery item across approximately 1,200 stores.
- STARTUP MODE: direct small commits to main, Production as practical QA, rapid fix-forward, no branch/PR/release ceremony unless Chris asks.
- Deep reviews are read-only by default. Recommendations remain proposals until approved.
- Product north star: bespoke “Pagani, not Ford” tool for Lisa, a solo luxury broker. Deep machinery, narrow interface. Automatic capture protects selling time; Lisa must not become a data-entry operator.

LISA INFORMATION ARCHITECTURE
Working lens:
- Dashboard = Orient
- Attention/Catch-Up = Act
- Client = Remember
- Deal/Contract = Execute

This is a lens, not a rigid taxonomy. Canonical state stays normalized; Lisa’s read experience may be deliberately denormalized. Repeating a canonical fact is useful when it preserves flow or answers a different contextual question. Remove only repetition that answers the same question in the same context.

Do not recreate separate Activity or Showings destinations merely because those capabilities exist. A panel earns its keep when it improves another panel, supports a frequent decision, prevents a costly screen switch, stays current automatically, or justifies its visual/maintenance cost.

Approved visible CORE vocabulary while preserving internal NEXUS identifier:
- Cockpit → /portal/dashboard
- Clients → /portal/clients
- Catch-Up → /portal/attention
- Contracts → /portal/deals
- Cabinet → /portal/documents

OPPS, SUPPORT, and TECH retain their names. Workflows and Forms remain functional but hidden from visible CORE navigation. Portal logo returns to public site; MAIN top-nav item is removed. Public-site Portal link is the far-right final navigation item.

CURRENT GIT / PORTAL CHECKPOINT
Observed main HEAD before this continuity commit:
- f3bb41009b17eb41af9e60131d97bf8930c9afc6 — media-count casts, CORE nav, capsule top-nav, property-admin 2xl table breakpoint, rounded panels, and public Portal link.
Parents/reference points:
- da9d80246ecbc7ae1e793b3dc2d6830aaea2da97 — interaction-component facelift.
- 83228eb490503803f3edfccbaee6ce78cfeb1ea5 — UI Lab portal primitives.
- 72d72031d8bca9f9d9977f22f89a7f306eef8c23 — Marlowe Gmail artifacts.
- 5c9796596f1fa0057d90a96db4360ccb9b62433c — Attention/showings/activity, nav declutter, glass polish, Security rename.
- b3af37fa2c1abfa95e1ea45e814b0bcb27d2829e — this matching ARCH-01 Git checkpoint.

The facelift promotes custom portal primitives only where materially useful: tables, pagination, search, dialogs, and responsive cards. It preserves the custom Grok-built glass design and adds no component dependency. Verify current Vercel Production rather than assuming deployment from this checkpoint.

MARLOWE GMAIL / DATA INTAKE BOUNDARY
Artifacts:
- docs/marlowe-gmail-relationship-census-report-2026-08-24.md
- docs/marlowe-gmail-relationship-census-private-2026-08-24.csv

Partial evidence only: 7,696 messages, 2011-06-26 through 2013-12-31, 2,018 external identities; 115 two-way, 60 owner-initiated outbound-only, 1,843 inbound-only, 1,140 automated/bulk evidence. Connector results were nondeterministic/incomplete; no full census claim. No message bodies, subjects, attachments, or retained snippets.

Correct architecture:
source metadata extraction
→ source-faithful ODS/load layer
→ cleansing/classification
→ identity reconciliation
→ relationship mart or governed canonical promotion.

Highest-value first pass: person/name, email, phone, iMessage/WhatsApp reachability when available, provenance/source, first/last contact, direction/count metadata. Organization is weak enrichment. Most remaining Google metadata is noise/stale. A targeted second pass may summarize only high-value relationships later. Do not start with broad body ingestion.

Apple Contacts: 2,573 staged imported contacts are visible through Clients but remain non-canonical. Preserve the l_person/load projection versus person/person_identity boundary.

FROZEN ASSUMPTIONS / BOOTSTRAP
- Do not reorganize merely to eliminate overlap.
- FUB and Cloze are sources of ideas, not information-architecture authorities.
- Keep admin CRUD out of Lisa’s daily flow; OPPS stewardship may be designed later.
- Review Deal/Contract composition separately; do not mix it casually into portal polish.
- Next session: read docs/ARCH-01-README-SUPPLEMENT.md; query this ARCH-HANDOFF row; inspect current main and Production; reconcile any active builder report; then state facts, contradictions, and the smallest proposed move.
- A later explicit decision from Chris supersedes this checkpoint and must be durably recorded.


DATA SURFACE / PAGING STANDARD — AUTHORITATIVE HUMAN DECISION (2026-08-25)

Real Apple Messages evidence exposed a demo-scale assumption: /portal/clients hung with 2,351 persisted relationship-evidence rows. This establishes an application-wide invariant.

Every potentially unbounded data collection displayed by a UI must have a bounded read contract. Use server-side paging; apply filter/search/sort in SQL or the backing datastore before the page window; retrieve counts separately when needed; keep enrichment set-based/batched; and prevent inactive large tabs/panes from blocking active content. Default page size is 50 unless a surface explicitly justifies another bound. Prefer keyset/cursor pagination when practical; measured LIMIT/OFFSET is acceptable for ordinary navigation.

Do not hydrate entire growing datasets in application memory merely to render/search/sort/count a screen. Do not raise timeouts or delete/hide real rows as a substitute for fixing read-path scale. New data screens inherit this standard automatically; existing screens must be corrected as they are touched or when real volume exposes a defect. See ENG-34 for executable acceptance criteria.

================================================================================
THE SERVICE TIER AND THE MVI SCREEN RUNTIME - SETTLED ARCHITECTURE (AMENDMENT 2026-09-11/12)
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

A later explicit decision from Chris supersedes this amendment and must be durably recorded.