# Forge Workflow Architecture — from the one-line idea to the invariants

**Status:** durable explainer. Owner: whoever holds the engine. Written 2026-09-13 from a session
that fixed nine defects in one day; every law below was paid for.

**Read this if** you are about to touch `workflow_app/`, `workflow_app/forge/`, the FORGE_SDLC XML,
the publish path, or any screen that claims to show what the engine did.

**Provenance.** Three kinds of statement appear here and they are marked:

- **[built]** — code exists in this repo, with the file named.
- **[measured]** — a number was observed, not estimated; the probe or test is named.
- **[proposed]** — a design not yet implemented. Never treat as fact.

Papers behind the numbers: MAP (*Measuring Agents in Production*, arXiv 2512.04123), Meta's
system-vs-model scaling work, and AgentRx (failure attribution). Their claims are labelled where
used; anything unlabelled is this repo's own observation.

---

## Part 0 — The whole thing in one paragraph

Forge turns a normalized story into a committed, verified, published change by running a
**workflow**, not a prompt chain. A workflow engine owns *when* (an XML definition of nodes and
transitions), a team map owns *who* (role → player → model/harness), and Forge owns *how*
(commands that dispatch one role, in one worktree, and collect evidence rows). The engine's
workflow instance is the canonical record of what happened. Nothing else — not Slack, not a
model's reply, not a dashboard cache — is allowed to be the source of truth about the run.

If you remember one sentence: **the failure mode is the channel, not the parser.**

---

## Part I — Simple: what Forge is

### The three layers, and the rule that keeps them separable

```
PROCESS     FORGE_SDLC-v6.xml          "What needs to happen next?"
   |        owns nodes, transitions, terminals, routing decisions
   v
TEAM        team.ts                     "Who performs this responsibility?"
   |        maps responsibility -> player -> model/harness/profile
   v
EXECUTION   Forge / OpenCode / model    "Run it."
            owns the worktree, the tool call, the commit, the evidence row
```

**The XML knows positions, not providers.** Good: `responsibility="smith"`,
`command-type="forge.run_smith"`. Bad: `agent="deepseek-v4"`, `harness="opencode"`. A provider
name in a process definition welds today's model choice into the process, and the process is the
part that should outlive the model. **[built]** — this is the existing V6 shape; keep it.

### The six roles, one line each

1. **SCOUT** — find the truth on the ground. Read the repo, the packet, the live database facts.
   Produces findings, not code.
2. **ARCHITECT** — decide the shape: interfaces, boundaries, decomposition, and whether the story
   may be split at all.
3. **LEAD** — decide the route: SOLO / SMITH / ASSAY / HOLD, plus the work order.
4. **SMITH** — write the change in its own worktree. One bounded packet. Never invent siblings.
5. **QA / ASSAY / INSPECTOR** — run the story's frozen proofs and return a verdict, PASS or FAIL.
   QA carries no git identity and freezes no commit; adversarial by design, its job is to refuse.
6. **DEV_OPS** — release: migrations, publish, deploy, verify. Owns the PROD schema gate.

### Normal delivery

```
SCOUT -> ARCHITECT -> LEAD -> SMITH -> QA -> DEV_OPS -> COMPLETE
```

That is the happy path and it should be boring. Most of the work of making Forge real was making
this path boring.

---

## Part II — Advanced: the machine as built

### Where it lives

- Definition: `workflow_app/definitions/FORGE_SDLC-v6.xml`, loaded through
  `workflow_app/forge/forge-executor.ts`. **[built]**
- Roles are phase agents (`workflow_app/forge/agents/role-agents.ts`) that `collect()` evidence;
  the parent gate (`forge-phase-agent.ts`) stays the decider. **[built]**
- A role runs in its own worktree under `~/Documents/Culebraluxe-worktrees/<story>-<id>-e0`
  through the OpenCode harness (`agent-runtime/opencode/`). **[built]**
- Runs execute against **PROD only**; the environment is not something a run may flip
  (`docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` §0). **[built]**

### Work items: the unit of "who is doing what"

A **work item** is a claimable row (`db/agent-work.ts`). Claiming is exclusive and
recovery-aware: a claim older than 15 minutes can be interrupted through the engine's own recovery
path, and `pnpm forge:clean` does exactly that so a run never reads another run's leftovers.
**[built]** `forge-claim-blocker.ts` names *why* a claim is blocked instead of silently refusing.
**[built]**

### The snapshot contract: what a role may decide from

Decisions are made against a **frozen contract** (`db/forge-run.ts`): `*_snapshot` fields,
`run_phase`, `lead_decision`, `packet_sha`, machine counters. If the story packet changes on disk
while a run is in flight, the snapshot says so — a stale packet is a stale packet, and the run
must not silently continue against a different story than the one it was handed. **[built]**

Doctrine detail: `forge_role_contract` (migration 170) carries the decision, `forge_role_plan_chunk`
/ `forge_role_assignment` (171) carry the plan. Reply markers (`LEAD_ROUTING:`,
`FORGE_ARCHITECT_HANDOFF:`) remain as the **fallback** for rows never written: **fields win, text
is the fallback.** `lead-proposal-resolve.ts` is the ONE seat for the Lead decision, so a refusal
cannot be re-reviewed and accepted by a second evaluator. **[built]**

### The publish path — the fix that mattered most

```
QA PASS (the verdict)
   |
   v
is remote main an ancestor of the candidate?
   yes -> publish (fast-forward, never force)                 -> published
   no  -> integrate in a scratch worktree:
            - detached worktree AT THE CANDIDATE (candidate never rewritten)
            - merge the CURRENT remote head into it
            - RE-RUN the story's frozen proofs on the integrated tree
                pass -> publish the integrated commit         -> integrated-and-published
                fail -> publish nothing, name the reason      -> integration-unverified
            - real conflict -> fail closed                    -> integration-conflict
```

**A clean merge is not proof.** A merge can apply without conflict and still break the story, so
the integrated commit is re-verified before it may become main. `--force` is never used, anywhere,
by anything. **[built]** — `lib/worker-workspace/publish.ts`, wired at
`workflow_app/forge/db-release-executor.ts`; the proofs are the story's own frozen commands parsed
by the same parser the Lead and Assay use, never model prose.

Why it mattered: the original publish was fast-forward-only *by design* ("no merge/rebase/PR
ceremony exists here"). With main moving several times an hour, **every** candidate provisioned
from anything but the newest main was unpublishable forever — while every individual gate kept
reporting success. That produced the pass → repair → turn-cap loop that looked like model
stupidity and was actually arithmetic. **[measured]** 2026-09-13.

### The candidate commit is the release unit

Smith's commit is the candidate: one exact commit, never "the current tree". QA does not verify a
SHA and carries no git identity — its verdict is PASS or FAIL over the story's frozen proofs.
DEV_OPS publishes the candidate only when that verdict passed; when main has moved, the publisher
merges the candidate, re-runs the frozen proofs on the integrated tree, and publishes that commit.
Nothing recomputes "the current candidate", because "current" is a moving target. **[built]**

### Budgets and counters above every door

- **Turn cap** — one integer per generation, default 10, enforced ABOVE every other door ("DOOR
  ZERO") because a door you cannot reach thanks to a long loop is not a door. At the cap the turn
  is not dispatched and the run stops with `GENERATION_TURN_CAP`. **[built]**
  `model-turn-budget.ts`; the count is a fact (`countForgeGenerationTurns`), not an estimate.
- **Dispatch ledger** — `forge_dispatch_score` (migration 175) records what was dispatched to whom.
  **[built]**
- **First violation** — `first_viol` records the first rule a generation broke (migration 176,
  `first-violation.ts`). **[built]**
- **Cost ceiling in tokens/dollars** alongside the turn cap. **[proposed]**

### Measured facts (do not re-derive these)

1. One role's model call is about **39 seconds**; role wall clock measured **52 seconds**
   (2026-09-13). Harness startup is 0.30s and config load 0.55s, so startup is not the cost —
   inference is. **[measured]**
2. The `rtk` shim used to fork-bomb every tool call (4,627 processes, load 34, ~80s blocked per
   call). Fixed in `forge-tool-seams.ts`; a regression test fails if it returns. **[measured]**
3. PROD engine volume at 2026-09-13: **54 process_instances, 1,775 process_events, 1,287 captured
   app_errors**, newest instance `2026-09-14 01:20Z`; DEV was stale since **2026-09-10** with 7
   instances. **[measured]** `scripts/probe-env-topology.ts`
4. Production agents are short and boxed: MAP found 68% cap at ≤10 model steps, ~80% run a
   predefined workflow (which Forge is in), and reliability is bought with sandboxes and
   checkpoints rather than a smarter model. **[paper → policy]**

---

## Part III — PhD: the laws

These are not style preferences. Every one of them exists because a run produced a *confident
wrong answer* without it. Nine defects were fixed on 2026-09-13; their shapes reduce to these laws.

### Law 1 — The failure mode is the channel, not the parser

A decision must never live only in a model's chat reply. If the only record of a decision is prose
in a transcript, then a truncated reply, a retry, a second reader, or a re-review can each produce
a different decision from the same run. Decisions travel in **rows**; text is only a fallback for
rows that were never written. **[built]** — `forge_role_contract`, `lead-proposal-resolve.ts`.

### Law 2 — A refusal must name its cause

Eight of the nine defects shared one shape: **a refusal that did not say why.** A gate returned
"no" and the orchestrator answered by trying the same thing again, because nothing told it what
was actually wrong. Any code that refuses must say *which* condition refused, *what value* it saw,
and *what would change the answer*. This is why outcomes are named (`integration-unverified`,
`GENERATION_TURN_CAP`, `PUBLISH_CONFLICT`) instead of booleans. **[built]**

Corollary: an unnamed failure is a bug even when the refusal itself is correct.

### Law 3 — A computed value must be wired to its reader

The other repeated shape: something *was* known and nothing displayed it. `dbTargetInfo()` has
reported the resolved database target, Neon branch, and declaring variable for weeks while no
engine screen called it — so an operator reading an empty recorder concluded the engine was idle,
when the engine's history was 54 instances and 1,775 trace events in the other environment. Call
this the **parallel-dimension defect**: the system is honest and the operator is misled anyway.
Whenever you compute a fact for a human, wire it to the screen in the same change. **[built]** —
recorder header, 2026-09-13.

### Law 4 — One environment declaration, and no default

There is no implicit target. `APP_ENV`/`VERCEL_ENV` must declare it or resolution **throws**;
`ForgeDB` refuses to open a connection rather than fall back to the other environment. The old
DEV default is precisely how a Forge script wrote DEV while the board read PROD. **[built]** —
`lib/execution-target.ts`, `db/database-gateway.ts`, `db/forge-db.ts`.

### Law 5 — A release names one exact commit

See Part II. Verifying "the current tree" is verifying nothing. Smith's commit is the candidate and
DEV_OPS publishes that commit — but the gate is QA's verdict (PASS or FAIL over the frozen proofs),
not a SHA QA holds. **[built]**

### Law 6 — A retry must be able to finish

The governing principle of the publish fix. If a system can enter a state where *no* retry can
succeed, it is not a strict system, it is a broken one — and the turn cap will blame the model for
the arithmetic. Before adding a guard, ask: **is there a reachable successor state?** A
fast-forward-only publish whose base has moved has none. **[built]**

### Law 7 — Idempotency is a workflow property, not an agent property

Workflow engines retry. Every agent command needs a durable execution key
(`workflowInstanceId + nodeId + attempt`) so a retry recognises prior work instead of spawning a
second Smith or republishing a commit. **[proposed]** — design recorded in Part V, item 5.

### Law 8 — Budgets are counted in the resource that drains

Turns are a proxy; tokens and dollars are the resource. Cap what actually depletes, enforce it
above every other door, and make no value of the variable mean *unlimited*. **[partly built]**

### Law 9 — HOLD is a state, not an error

"Machine deliberately stopped" must be durable, with `holdReason`, originating node, evidence, and
the event that resumes it. Conflating a deliberate stop with a failure makes both unreadable.
**[built in spirit]** — the existing Hold behaviour is one of Forge's best characteristics; keep it
first-class as the workflow grows.

### Law 10 — One orchestrator, one truth

Two state machines (say Forge's Run plus an external framework's Flow) create a question with no
answer: "the Flow says Running while the Run says Hold — which is true?" Days were spent deleting
this category of split truth. Do not reintroduce it. **[doctrine]** — see Part V on CrewAI.

### Law 11 — Normalization belongs at the repository boundary

Code above `db/` must never need to know whether the driver returned a `Date`, `BigInt`, `Buffer`,
or a driver object. Normalize on the way out, and test the real driver shape at real cardinality —
several of these defects only execute at higher row counts. **[built]** — repo operating rules.

### Law 12 — Bounded work must say it is bounded

A read capped at 20 rows must surface "showing 20 of N", or the cap becomes a new silent refusal
(Law 2) wearing a performance costume. **[partly built]** — the recorder's 20-sibling cap currently
hides the omitted count; this is a known violation, recorded here rather than forgotten.

---

## Part IV — Failure classification, and why repair is not a loop

**The build path and the failure path must not be the same loop.** QA-fail → Smith again is the
weakest possible repair policy: it assumes every failure is a code defect and asks the same worker
to guess what the reviewer meant. A real repair path classifies first.

```
                   +--------- QA FAIL ---------+
                   |                            |
                   v                            |
             CLASSIFY FAILURE                   |
        +---------+---------+---------+         |
        v         v         v         v         |
      SMITH   ARCHITECT    SCOUT    DEV_OPS      |
     code bug   design    unknown  environment   |
        |         |         |         |         |
        +---------+---------+---------+         |
                   |                            |
                   +------------> QA -----------+
```

The classification is a **fact**, not prose. QA and DEV_OPS return a structured outcome; the XML
routes on it. **[proposed for V7]** — the classes:

| Class | Meaning | Repairs with | Today's evidence |
| --- | --- | --- | --- |
| `CODE_DEFECT` | the change is wrong | SMITH | Assay refusal line |
| `ARCHITECTURE_GAP` | the design is wrong or missing | ARCHITECT | Lead `HOLD` + reason |
| `REQUIREMENTS_GAP` | the story is ambiguous or incomplete | SCOUT (or back to the author) | Lead `HOLD`, packet contradiction |
| `UNKNOWN_CAUSE` | nobody can name it yet | SCOUT | repeated identical failure |
| `ENVIRONMENT` | the machine, not the code | DEV_OPS | env/claim/readiness probes |
| `MIGRATION` | schema not applied/verified | DEV_OPS | `pnpm db:parity` / `db:migrations` |
| `PUBLISH_CONFLICT` | identity/lineage refused at publish | DEV_OPS (now: integrate+re-verify) | `failureClass: 'PUBLISH_CONFLICT'`, `failedReleaseStage: 'PUBLISH'` |

Two rules make this work in practice:

1. **Classification must be falsifiable.** "Unknown cause" is honest and useful; a wrong confident
   label is worse than `UNKNOWN_CAUSE`, because it dispatches the wrong specialist.
2. **The first violation is the useful one.** Later failures are usually downstream of the first —
   which is why `first_viol` (migration 176) records the *first* rule broken, not the loudest.

---

## Part V — V7: what to add, and what to deliberately leave out

The seven additions below are the author's V7 list, carried here with status. The headline stays
**workflow-driven Forge** — this is not a rebuild.

1. **Process-definition selection.** Build the *mechanism* to select a definition (so
   `FORGE_FEATURE`, `FORGE_BUG`, `FORGE_RESEARCH` can exist later) while shipping only
   `FORGE_SDLC-v1`. Do not bake "one Forge process" into the adapter. **[proposed]**
2. **Failure classification as facts** — see Part IV. **[proposed]**
3. **A real HOLD state** — durable, with reason/node/evidence/resume event. Not an error. **[partly built]**
4. **DEV_OPS as a subprocess** — publish → migration required? → DEV migrate+verify → PROD
   migrate+verify → deployment → smoke → COMPLETE. A future non-Vercel/non-Neon project then gets
   a different release subprocess without touching the SDLC. **[proposed]**
5. **The candidate commit as a tested invariant** — Smith's commit is the release unit and publish
   names it; V7 acceptance tests proving it (Law 5). **[built in code, not yet tested as an invariant]**
6. **Idempotent/resumable agent commands** — durable execution key
   `workflowInstanceId + nodeId + attempt`; a retry must recognise prior work rather than spawn a
   second Smith or republish (Law 7). **[proposed]**
7. **Workflow-native telemetry** — Slack/OpenClaw becomes an *observer of workflow transitions*
   instead of Forge deciding what status to send. Then the message reads
   `ENG-123 → Architect complete → Lead → SMITH → Smith candidate 9d18f3a → QA PASS → DevOps
   deploying → Production verified → COMPLETE`, and the workflow instance is the canonical story.
   **[partly built]** — the mirror exists (`agent-work.ts`) and is correctly fail-open and
   redacted, but it carries role/profile/SHA only: no frozen-contract pointer, no packet-stale
   flag, no publish preview.

### Leave out of V7 (deliberately)

- **Multi-Smith parallelism** — the fork/join foundation exists; rushing it is how you get three
  fast people making a mess (see Part VIII).
- **Adaptive model selection, dynamic team creation, CrewAI ingestion, agent negotiation,
  self-modifying workflows, A2A.** Fascinating *after* you have execution history. Before that,
  they make it impossible to tell whether the workflow migration itself works.

---

## Part VI — CrewAI: steal the patterns, do not surrender the architecture

Reviewed OSS offering versus Forge, layer by layer. The overlap is substantial:

| CrewAI | Forge |
| --- | --- |
| Agent roles | Scout / Architect / Lead / Smith / QA / DEV_OPS |
| Crews | Team Map |
| Flow | Story Run / run state machine |
| Tasks | Work Items |
| State | Neon durable state |
| Planning / delegation | Lead PRE + decomposition |
| Model selection | player/provider/model/harness routing |
| Recovery | declarative recovery policy |
| Checkpointing | durable Run + append-only evidence |
| Execution logs | run/work evidence + flight recorder |
| Tool execution | harness/tool layer |
| Validation | QA / Assay verdict over the frozen proofs |
| Parallel agents | N-Smith architecture (not yet built) |
| Human control | Story Board / Hold / approval boundaries |

CrewAI recommends a **Flow-first** architecture for state, branching and observability — which is
the direction Forge arrived at independently by making the **Story Run** (not the agent) the durable
boundary. Two frameworks agreeing on that is worth noticing.

**Where they are genuinely ahead — study, do not adopt:**

1. **Checkpoint / fork / replay** — capturing runtime state, replaying from a step, forking with
   different inputs. Real design material for Forge Run recovery.
2. **Scoped memory** — memories carry scope/category/importance and retrieval blends semantic
   relevance, recency and importance, scoped per agent or shared. Plausible for
   `/project /architecture /decisions /failures /patterns /scout-findings` — for **Scout and
   Architect only**. Smith's fuzzy memory must never override a Story Run contract.
3. **MCP / tool abstraction** — Forge hand-built its GitHub/Neon/harness layer; the ecosystem could
   save effort at the edges.
4. **A2A** — interesting only if Forge someday coordinates *outside* Forge.

**Why not ingest it now (Law 10):** you would run two orchestration state machines, Forge's Run and
CrewAI's Flow, and the first incident would pose an unanswerable question — "Flow says Running, Run
says Hold, which is true?" That is the split-truth category this repo spent days eliminating.

**Where Forge is genuinely ahead:** CrewAI is a general-purpose agent framework; Forge is a
software-delivery machine with authority boundaries. The intellectual property is the chain
`Story Contract → Story Run → Smith's candidate commit → QA verdict (PASS/FAIL) over the frozen
proofs → DEV_OPS publishes the candidate → deployment + migration evidence → Complete`. CrewAI can orchestrate agents.
Forge knows what it means for software to be **truthfully shipped**.

Discipline: finish proving V6.1 and the six-role topology, then do a deliberate gap review —
probably **4–6 architectural stories**, not a framework migration.

---

## Part VII — The plumbing punch-list (close before visibility work)

Visibility built on ambiguous plumbing displays confident lies (Law 3). Order matters.

### P0 — close before any visibility story

1. `FORGE_PROVIDER_BUILDER_FLASH` read mid-loop → explicit `builderFlashOverride` parameter at the
   call boundary, applied as a named team variant in `team.ts`. Today launchd and a shell can route
   differently, so the Portal would display one harness while the run used another.
2. `PolicyDeepSeek/OpenCode.resultExternal` duplication → one `commitAndRevokePolicyEvidence()`
   helper. The candidate-commit seam must not drift between harnesses (Law 5).
3. Boot-time `throw` for an unused `pi` mapping → a blocked `pi-harness` placeholder. One dead map
   entry currently kills all hydration.
4. DeepSeek readiness uses `existsSync` (no `X_OK`) plus unconditional delegated auth → uniform
   `commandIsInstalled` plus explicit auth qualification, matching OpenCode/the gateway. Otherwise
   the "ready" badge is optimistic on one host and pessimistic on another.

### P1 — immediately after P0, small and safe

5. `CORE_CAPABILITIES` vs `WRITE` vs lane caps (three names, one privilege set) → delete `CORE`;
   use `WRITE_CAPABILITIES` / `ASSAY_CAPABILITIES` from `lanes.ts` everywhere.
6. Silent capability union on shared profiles → fail closed on cross-lane reuse with different
   `requiredCapabilities`. `smith+night` sharing `builder-flash` is safe today; the widening trap is set.
7. `defaultDeepSeekConfig()` baking `process.cwd()` at call time → require an explicit `workspace`
   from `buildAgentInvokerWorkspaces` (which knows `worktreePath`); homedir fallback for local CLI only.
8. Registry rebuilt per hydrate (and again inside `gateSmithEnvelope`) → memoize per process,
   construct once in `forge-orchestrate-wake.ts`, pass down.
9. `deepseek-pro` player: vague `model:'pro'` plus a duplicated caps array → exact model id, deduped.
10. Inspector lineage `deepseek-judgment` collides with smith upgrade/emergency → dedicated
    `deepseek-review`, plus a `team.test.ts` assertion `inspector.lineage ∉ smith grades`.

**Gate for the punch-list:** `tsc --noEmit` clean, `git diff --check` clean, scoped `tsx --test` for
team + lane-policy + readiness + opencode-routing + gateway/provider + orchestrate* + handoff green,
`next build --webpack` green, no `next-env.d.ts` noise.

### The visibility story that follows — `ENG-FORGE-V6-VIS`: one truth, three lenses

Portal TECH, Slack and the CLI must read the **same seams**, or the operator gets three different
stories about one run.

1. **Portal TECH** — a Run Contract panel on story detail: frozen `*_snapshot` vs live story diff,
   `packet_sha` stale badge (Neon sha ≠ Git sha), `base_commit_hash` + `origin/main` tracking state,
   Lead `SOLO/SMITH/ASSAY/HOLD + reason` timeline from `lead_decision`/`run_phase`, machine counters
   and `failure_code`. Reuse `getForgeRunExecutionStory` / `getForgeRunMachineEvidence` — no new queries.
2. **Slack** — extend `ForgeSlackContext` with `packetShaStale`, `leadDecision/reason`,
   `publishPreview: publishable|conflict|no-candidate`; still fail-open, still redacted, same four
   event types.
3. **CLI** — `pnpm agent:workspace doctor` (wrapper sha match, `origin/main` resolve, orphan worktrees
   vs active items, registry readiness per profile) and `pnpm agent:work --preview-publish <story>`
   (dry-run `merge-base --is-ancestor` plus `verified == candidate`, no push).

**Swarm stays V7:** keep `SPLIT:n` parsed but Hold-gated (`LEAD_SPLIT_REQUIRES_MULTIWORKER`), collect
`lead_split_count` as future parallelism data, and never auto-prune on publish — orphan detector plus
manual `remove` only.

---

## Part VIII — Swarm: the last layer, not the next one

**Decomposition is an Architect authority, never a Smith's.** Before parallel work, only the
Architect may create child work items. The Architect decides: whether the story is safe to
parallelize at all; how many work items exist; which files/domains each owns; dependencies between
them; which shared interfaces must be frozen first; the merge order; and what "combined acceptance"
means after all items finish. A Smith receives a bounded packet and builds only that packet.

```
Story
  |
  v
Scout
  |
  v
Architect
  +-- SERIAL ---> Smith -> Assay
  |
  +-- PARALLEL -> Work Item A -> Smith A -> local verification
                  Work Item B -> Smith B -> local verification
                  Work Item C -> Smith C -> local verification
                        |
                  controlled integration
                        |
                  Assay on the WHOLE STORY     <-- non-negotiable
                        |
                  Complete / Hold
```

**The whole-story Assay is non-negotiable.** Parallel work introduces a failure class serial work
cannot have: two builders can each be individually correct and still produce a broken story, because
they touched the same seam, assumed incompatible things, or satisfied different readings of the same
acceptance criteria. Without a post-integration Assay you get `A passes, B passes, C passes, combined
system breaks` — the classic parallel-agent trap.

**The five things to vet before swarming:** Architect decomposition authority · file/contract
ownership · a real dependency graph · an integration gate (child completion ≠ story completion) ·
whole-story Assay against the original criteria. The repo already holds the key invariant:
**Work Item Done ≠ Story Complete.**

**Start smaller than "swarm":** first experiment = **two** parallel Smiths on a story with an
obviously clean split (UI component A and isolated service B with a frozen interface between them) —
never two agents editing workflow-engine semantics at once. Compare against the same work done
sequentially. If 2-way is clean, go to 3–4; only then consider 10+.

And before any of it: **finish dialing the serial path in as if parallelism did not exist** —
provider gateway correctness, execution-target safety, install/auth/config handling, clean
Scout → Architect → Smith → Assay semantics, packet completeness, commit ownership, evidence
capture, Hold/repair behaviour, TECH visibility, and one real end-to-end story on a non-DeepSeek
provider. Do not use parallelism to hide flaws in the machine: once the serial path is boringly
reliable, parallel becomes a performance feature instead of a new source of chaos.

---

## Part IX — How to read the machine when it misbehaves

Run this ladder before forming a theory. Every rung exists because it was the answer at least once.

1. **Which database am I looking at?** The screen or deployment declares it (recorder header).
   Engine runs write PROD; a local dev server declares `APP_ENV=development` and reads DEV. An empty
   screen may be a truthful DEV view of a busy PROD engine. `scripts/probe-env-topology.ts` prints
   host + database per target, and says so when two targets are the same database.
2. **Clear the control plane.** `pnpm forge:clean` cancels stale work items, interrupts stale claims
   through the engine's own recovery path, and aborts stale instances — an engine read against
   another run's leftover claims is not evidence. Then
   `pnpm forge:story:reset <story> reset --force`.
3. **Which door refused?** Named outcomes beat grep. `GENERATION_TURN_CAP`, `PUBLISH_CONFLICT`,
   `integration-unverified`, `integration-conflict`, claim blockers, `HOLD` + reason.
4. **How many turns has this generation spent?** Above the cap the turn was never dispatched, and
   the real fault is an earlier door.
5. **Which commit is the candidate?** Smith's commit is the candidate, and publish names the commit
   it actually published. An integrated publish legitimately publishes a different commit, and that
   tree's frozen proofs were re-run. An unnamed commit means identity drifted and every report since
   is suspect.
6. **Read the first violation, not the last error.** `first_viol` (migration 176); later errors are
   usually downstream.
7. **Only then** read the flight recorder for the instance, and if a screen looks wrong, check
   whether the value it shows was ever wired to the thing that computes it.

---

## Part X — Glossary, and the order of operations

### Glossary

- **Story** — a bounded unit of intended change, normalized into a packet
  (`docs/agent/packets/<ID>.md`) with a contract: scope, acceptance, evidence.
- **Story Run** — the durable record of one attempt to deliver a story; the boundary that owns
  state. Not the agent.
- **Work Item** — a claimable assignment ("who is doing what"); exclusively claimed with a
  recovery path for stale claims.
- **Generation** — one trip around the route (Lead decision → implementations → Assay → release),
  counted against the turn cap.
- **Role / Position** — what work must be performed (Scout, Architect, Lead, Smith, QA, DEV_OPS).
  The XML names positions.
- **Player / Profile / Lineage** — who performs it, with which capabilities, model and harness.
  The team map names players.
- **Candidate** — Smith's exact commit, the release unit from commit through publish. QA carries no
  sha; its verdict is PASS or FAIL over the frozen proofs.
- **Assay** — verification of the story's frozen proofs; the verdict is PASS or FAIL, and QA carries
  no git identity.
- **HOLD** — a durable deliberate stop with a reason, an originating node, evidence, and a resume
  event. Not an error.
- **Packet SHA** — the identity of the story contract at snapshot time; a mismatch means the run
  was decided against a different story than the one on disk now.
- **Flight Recorder** — the console over the engine's transaction/instance/event read model.
- **DOOR ZERO** — the turn-cap check above every other door, because an unreachable door is not a
  door.
- **Parallel-dimension defect** — the system is honest and the operator is misled, because a
  computed fact was never wired to the screen (Law 3).

### The order of operations (do not reorder casually)

1. Make the **serial** path boringly reliable (Part V punch-list, then one real end-to-end story).
2. Make **one truth visible** in three lenses (Portal TECH / Slack / CLI) — reading the same seams.
3. Add **failure classification and repair routing** so a failure dispatches the right specialist.
4. Add **Architect-controlled decomposition** (V7), gated and Hold-aware.
5. Only then **fan out workers**, 2 → 3–4 → more, with a whole-story Assay after every integration.

Each step makes the next one debuggable. Invert that order and parallelism hides the flaws in the
machine underneath it — which is how a factory becomes several very fast people making a mess.

### Related documents

- `docs/agent/CURRENT.md` — the live machine, short form.
- `docs/agent/MEMORY.md` — durable decisions that must outlive a tool.
- `docs/agent/DEV-OPS-DATABASE-PLAYBOOK.md` — environment topology and promotion order.
- `docs/agent/STORY_EXECUTION_CONTRACT.md` — the contract a story must satisfy.
- `docs/FORGE-V2.md`, `docs/FORGE-V3.md` — historical; superseded by the six-role engine.








