# CULEBRALUXE WORK ORDER — ENG-FORGE-WARM-SESSION-01

**Title:** One warm session per execution generation  
**Repository:** `culebraluxe/Culebraluxe-web`  
**Branch:** `main` only  
**Environment:** DEV proof only  
**Work type:** Forge execution model  
**Test policy:** Targeted only.

Filed: 2026-09-13. Source: operator cost/latency incident — Architect 12–30 min for one story; full job ~30 min; seed tax ~2 min per role death.

---

## Verdict (read this first)

The split-domain idea is **not** broken.  
The **process model** is.

Roles as *contracts + validators + artifacts* work.  
Roles as *separate cold model processes that each re-seed git and die* do not.

Session-passing as text between agents is a lossy photocopy of a KV cache. It will never repay the 2-minute seed. Keep one process alive. Write contracts to Neon. Change the prompt prefix, not the process.

This packet does **not** delete Scout / Architect / Lead / Smith / Assay.  
It deletes **one OpenCode process per role**.

---

## What actually works in the field

Observed pattern that survives contact with cost:

| Pattern | Who uses it | Why it works |
|---|---|---|
| One long coding session + tools + files | Claude Code, Cursor, Codex, Warp | Prefix/KV cache stays hot. Repo is already in context. |
| Artifacts-first multi-agent | Forge fielded contracts, legal/review desks | Handoff is a row, not a monologue. |
| Warm worker pool, role is a prompt | Production agent runtimes | Seed once. Turn N is cheap. |
| Cold start per specialist | Most CrewAI / “5 agents in a loop” demos | Looks clean. Burns tokens. Dies at 30 min/job. |

Nobody who is paying the bill keeps five cold specialists for one story.

Split *knowledge* (system prompt + allowed tools + write surfaces).  
Do not split *process* until a SPLIT is explicit.

---

## Cost model of the current path

```
seed(Scout)     ~2m   + discovery tokens
seed(Architect) ~2m   + re-read git the Scout already read
seed(Lead)      ~2m   + re-read the same tree
seed(Smith)     ~2m   + re-read again
seed(Assay)     ~2m   + re-read again
────────────────────────────────────────
startup tax     ~10m before any judgment
Architect turn  10–30m because it rediscovers instead of judging
```

The Architect you already have is not 10× dumber than a warm session.  
It is 10× colder.

ROI rule:

> Pay seed once per execution generation.  
> Every later role turn must hit a live prefix cache or it is a bug.

---

## Target operating model

Reuse ENG-FORGE-CONVERGENCE-01 identity. Do not invent a second orchestrator.

```
Story
  ↓
Execution generation E0          workspace key = <processInstanceId>-e<replanAttempts>
  ↓
ONE OpenCode session             seeded once (Scout collect)
  ↓
role turns in the SAME session
  Architect collect/decide  →  forge_role_contract + plan chunks
  Lead PRE                  →  SOLO | SPLIT | HOLD  (fields, not JSON)
  Smith                     →  candidate SHA
  Lead POST / QA / Assay    →  exact-SHA verdict
  repair                    →  same session, same workspace, new candidate
  REPLAN                    →  kill session, new generation, new session
```

Rules:

1. `workerId` is who runs. It is not Git lineage and not session identity.
2. Session identity = execution generation.
3. Role identity = prompt prefix + allowed tools + write tables.
4. Handoff identity = Neon rows (`forge_role_contract`, `forge_role_assignment`, `forge_role_plan_chunk`, candidate SHA).
5. Text session export is evidence, not the handoff.
6. SPLIT is the only reason to start a second writable session/worktree.

---

## Scope (smallest change that stops the bleed)

### A. Session ownership

Primary seam: `agent-runtime/invoker.ts` and the OpenCode client wrapper.

- Record the OpenCode session id on the first successful run of a generation (also satisfies ENG-FORGE-COST-01 precondition).
- Later role turns in that generation call `opencode run --session <id>` (or the keep-alive equivalent already used by the harness).
- If the session is dead, start one new session and treat it as a cache miss, not a new architecture.
- REPLAN / new generation always starts a new session.
- Do not pass “session” by stuffing Scout stdout into Architect stdin.

### B. Scout once

Scout is a **session boot**, not a peer in the roster.

- First turn of E0: Scout collect only (git, declared surfaces, packet, constraints).
- Persist Scout findings as rows / files the later prefixes can name by path.
- Architect must not re-walk git for facts Scout already wrote.
- If Architect needs a missing fact, it asks for a bounded re-collect, not a new process.

### C. Role = prefix, not process

Keep `workflow_app/forge/agents/role-agents.ts` collect/decide.

Change the runner so:

- collect/decide of Architect → Lead → Smith → Assay share the live session;
- each turn prepends only the role contract (allowed writes, fail-closed seams);
- the model is told “you are now Lead PRE; do not rediscover; read these row ids.”

Kill the JSON reply-marker path as the primary channel. Fields already exist. Use them. JSON parse is fallback-only and must log `channel=legacy_json` so it can be deleted.

### D. FAST is the product of this packet

ENG-FORGE-CONVERGENCE-01 Scope C stays the eligibility gate:

Eligible FAST: Smith + deterministic QA + publish.  
No Architect/Lead model turns.

Warm session makes FEATURE affordable. FAST makes small work cheap. Both stay.

### E. Stop condition against the 30-minute death spiral

Already specified in CONVERGENCE-01. Enforce it here on the session:

- same candidate SHA + same machine QA class ⇒ `HOLD / NO_PROGRESS`
- Architect wall clock > N minutes with no contract row written ⇒ abort the turn, keep the session, emit incomplete
- never start a fifth cold seed to “try Architect again”

---

## Explicit non-goals

- Do not delete role names, TECH labels, or the roster UI.
- Do not build a second queue or “agent mesh.”
- Do not implement true cross-process KV cache sharing. Vendors do not give you that.
- Do not convert widgets to dollars.
- Do not edit FORGE_SDLC-v3.xml in place.
- Do not auto-merge to main.

---

## Acceptance criteria

1. One FEATURE execution generation starts **at most one** OpenCode session under happy path.
2. Architect → Lead PRE → Smith → QA in that generation reuse that session id.
3. Seed / first-token time is paid once; subsequent role turns do not pay a full 2-minute seed.
4. Architect writes `forge_role_contract` + plan chunks without a mandatory full-repo rediscovery when Scout rows exist.
5. Lead decision is read from fields first; JSON marker parse is fallback and telemetry-tagged.
6. Repair stays on the same session + same workspace + new candidate SHA.
7. REPLAN starts a new session and a new workspace key `<pid>-e<n+1>`.
8. Session id is persisted on the run (COST-01 prerequisite).
9. A targeted test proves two role turns in one generation share a session id.
10. A targeted test proves a dead session is replaced once, not once per role.

---

## Assay

```
pnpm exec tsc --noEmit
pnpm exec tsx --test agent-runtime/invoker-workspace.test.ts
pnpm exec tsx --test workflow_app/tests/forge-convergence.test.ts
# plus a new:
pnpm exec tsx --test agent-runtime/invoker-session.test.ts
```

Plus one DEV dogfood story timed with:

- t_seed
- t_architect_contract_row
- t_lead_fields
- t_smith_candidate
- session_id equality across those turns

Success signal: Architect contract row in minutes, not 12–30, on a story a warm session already understands.

---

## What to tell yourself at 1am

The idea that died last night was **cold specialists**.  
The idea that is still good is **named judgment with durable artifacts on a warm process**.

You already paid for the hard part: fielded contracts, fail-closed seams, exact-SHA QA, execution-generation workspaces.  
Those stay.  
The process graph shrinks to one live session per generation.

That is the whole move.
