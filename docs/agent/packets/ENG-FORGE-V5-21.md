# ENG-FORGE-V5-21 — Story-Resident OpenCode Session Continuity

**Status:** PLANNED — parked for the post-testing-cycle project. Do NOT build this
cycle. Research capture 2026-09-08 (OpenAI + operator). See `docs/agent/MEMORY.md`
"ENG-FORGE-V5-21" entry for the one-paragraph version.

## Goal

Fix the major cognitive weakness in Forge: each SDLC role currently receives a
fresh model session and loses the accumulated working model of the story.

Provide one native OpenCode durable TOP-LEVEL SESSION for the construction side
of one Forge story: Scout → Architect → Lead → Smith → Repair. The same OpenCode
session must survive those Forge role boundaries.

## Core invariant

> **CONTEXT PERSISTS; AUTHORITY DOES NOT.**

- Forge remains the outer SDLC control plane.
- OpenCode owns conversational/session continuity only.
- Do not collapse Forge roles. Do not let OpenCode route the SDLC.
- Do not share sessions across stories/worktrees.
- Do not expose the construction session to independent QA/Assay.
- Do not store a synthetic transcript in Neon.

## Research finding (drives the implementation)

Use a **top-level OpenCode session (`ses_...`)** as the Forge story brain.
Do NOT use OpenCode's Task/subagent `task_id` mechanism for phase continuation —
upstream OpenCode has reported stale-permission behavior when a `task_id` is
resumed under a different subagent.

Prefer the OpenCode **server/SDK session API** over repeated
`opencode run --session --model` CLI calls. Current OpenCode server session
messages support per-turn model / agent / system / tools-and-permissions. The CLI
has had active resume/model-switch regressions (e.g. `-m` ignored on resume).

## Target architecture

```
Forge Story
   └─ process instance / execution generation
        └─ one Forge serial worktree
             └─ one persisted OpenCode session binding
                  ├─ Scout      Flash / READ
                  ├─ Architect  Pro   / PLAN-READ
                  ├─ Lead PRE   Pro   / READ
                  ├─ Lead SOLO  Pro   / WRITE if Forge authorizes
                  ├─ Smith      Flash / WRITE
                  └─ repair     team-mapped model / phase permissions
             └─ candidate SHA freeze
                  └═ HARD CONTEXT WALL ═
                       ├─ Inspector FRESH context
                       ├─ Assay     deterministic / model-free
                       └─ DEV_OPS   separate
```

QA failure → engine routes back to repair → resume the existing construction
session. QA output is NOT inserted into that session automatically.

## Session ownership (persist binding/lifecycle only, never the transcript)

Suggested durable identity: `story_id`, `process_instance_id`,
`execution_generation` / `executionWorkspace.runId`, worktree identity/path,
`base_commit`, `opencode_session_id`, `opencode_version`,
`status: active | qa_frozen | retired`, `created_at`, `last_used_at`.

Uniqueness must prevent: another story using the session; another worktree using
it; another execution generation silently inheriting it. If Forge finds a binding
whose OpenCode session no longer exists, whose worktree differs, or whose
ownership cannot be proven → **HOLD**. Never silently create a replacement
continuation session.

## Role / model routing

Default Forge team map currently puts only Smith on OpenCode. That cannot give
native continuity across all four roles. Introduce a BOUNDED RESIDENT DOGFOOD
team/path first (do not destroy the disposable path):

- Scout → OpenCode / DeepSeek Flash
- Architect → OpenCode / DeepSeek Pro
- Lead → OpenCode / DeepSeek Pro
- Smith → OpenCode / DeepSeek Flash

Model selection stays owned by Forge `team.ts`. OpenCode currently pins its
adapter to DeepSeek Flash and throws on other models — generalize the resident
path so the team-mapped model is supplied explicitly for EVERY turn. Keep the
existing disposable path until dogfood passes.

## Permission contract

Forge permissions are authoritative. Derive the OpenCode effective permission set
from the current `AgentExecutionContext` capabilities/policy on EVERY turn — never
trust permissions stored from the previous OpenCode turn. Prove at minimum:
Scout/Architect read-only no-commit; Smith authorized-write + scoped tests, commit
policy Forge-owned; Smith → read-only revokes all write. Keep OpenCode
`task`/subagent DENIED. Do not use prompt wording as the security boundary where
OpenCode provides a real permission mechanism.

## Split rule

V5-21 dogfood is SERIAL. Do not reuse one session across `smith_split_work` child
worktrees — on a SPLIT path either HOLD resident continuation or stay on the proven
disposable behavior. Do not invent cross-worktree shared context.

## Likely code surfaces (inspect first)

`agent-runtime/opencode/opencode-client.ts`, `opencode-harness-adapter.ts`,
`opencode/*.test.ts`, `agent-runtime/factory.ts`, `team.ts`, `lanes.ts`,
`types.ts`, `workflow_app/forge/agent-runtime-role-runner.ts`.
Likely add `agent-runtime/opencode/opencode-session-client.ts`, a durable binding
repository + migration (`db/forge-opencode-session*.ts`, `db/migrations/*`).
Create this packet as the doc. Do not add another autonomous router/agent role.

## Phase 0 — qualify installed OpenCode (before any behavior change)

1. Record `opencode --version`.
2. Inspect the installed server OpenAPI contract (`opencode serve` / `/doc`) or the matching SDK types.
3. Prove experimentally: create a session → send a Flash turn → resume the SAME session with Pro → resume with Flash → earlier-session info survives both switches.
4. Prove permissions recompute between turns (READ → WRITE → READ).
5. Prove a write attempted during the final READ phase is denied.
6. If the installed version cannot satisfy these, HOLD with evidence. Do not invent transcript rehydration as a fallback.

## Acceptance tests

1. First construction phase creates one OpenCode session id.
2. Architect uses the exact same id.
3. Lead uses the exact same id.
4. Smith uses the exact same id.
5. Earlier-phase info is recalled later WITHOUT being repeated in the later Forge prompt.
6. Flash → Pro → Flash happens inside that same session.
7. Permission transition READ → WRITE works.
8. WRITE → READ proves actual write revocation.
9. Kill/restart the Forge adapter/process; the persisted binding resumes the same session.
10. A different story cannot use the id.
11. A different worktree cannot use the id.
12. QA/Inspector/Assay receives no construction-session id.
13. Failed QA → repair resumes the original construction session.
14. No OpenCode transcript is stored in Neon.
15. Existing disposable OpenCode tests continue to pass.
16. Existing exact-candidate / scoped Assay behavior is unchanged.

Targeted Assay: `pnpm exec tsx --test agent-runtime/opencode/*.test.ts agent-runtime/*session*.test.ts`
Add only the targeted tests required for changed seams. Do not run the full ~2h regression unless explicitly authorized.

## Live dogfood proof (one real test story)

During Scout, the session learns one unambiguous nonce fact that is NOT placed in
the later Architect/Lead/Smith prompt. Architect must demonstrate it; switch to Pro,
Lead still demonstrates it; switch to Flash, Smith still demonstrates it and does the
bounded change. Then switch to a read-only phase and prove an attempted edit is
denied. Terminate the Forge process between two phases and prove the session recovers
from its persisted id. Freeze candidate and prove independent QA has fresh context.

## Delivery

Return: exact files changed; migration if any; installed OpenCode version proven;
exact OpenCode session API used; permission mapping; model-switch proof;
session/restart proof; QA context-wall proof; targeted test output; dogfood
evidence; commit SHA. Do not claim success if any of those are simulated.

