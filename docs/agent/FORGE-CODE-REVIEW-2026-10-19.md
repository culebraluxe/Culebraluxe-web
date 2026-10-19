# Forge code review — how it actually works (2026-10-19)

Read order used: `CURRENT.md` → `WORKFLOW-ARCHITECTURE.md` → `FORGE-WORKSHOP.md` →
`FORGE-MODEL-GENERATION-BUDGET.md` → `git_publish.rs` → `harness.rs`/`worker.rs` →
`opencode_agents.rs` → `ORCHESTRATOR-IN-THE-LOOP.md` → `AGENT-OS-PHASE-1/2`.

## 1. The machine

```text
STORY / packet
  → process  FORGE_SDLC-v6.xml (nodes, transitions, service= bindings, gates)
  → reliability ForgeJobBridge/JobService (claim, lease, attempts, heartbeat, recovery)
  → service  seven AbstractForgeService roles (Scout, Architect, Lead, Smith, Inspector, Assay, DevOps)
  → runtime  OpenCodeHarness (model/process) in ONE workspace per generation
  → evidence Neon rows (role contract/plan/assignment, findings, HOLD, decisions, artifacts, tokens)
  → publish  git_publish.rs — the ONLY publish door — origin/main with integration proofs
```

- Binary: `forge` driven by `pnpm forge:engine`; worker/task/re-workflow beside it.
- Role ownership is **in the XML** (`service="forge.smith"`), not in Rust; `ForgeServiceRegistry`
  composes the seven services.
- Durable state is the contract: decisions travel in rows (`forge_role_contract`,
  `forge_role_plan_chunk`, `forge_role_assignment`); reply markers are only a fallback. "The failure
  mode is the channel, not the parser."
- One workspace per execution generation (`worktree.rs`); no per-role worktrees; git branch `agent/…`.
- Forge runs against **PROD only**; release/HOLD/QA boundaries are enforced in code and tests.
- Model policy already exists: `model_policy` (cheap/judgment) + explicit override, recorded per turn;
  model budget per generation (migration 286); harnesses: OpenCode (V2 profiles + advisory),
  DSH (headless, below-adapter model config), Maestro.

## 2. Where ForgeVCS has to plug in (the real seams)

1. **Publish door** — `forge/src/engine/git_publish.rs` is deliberately the only path a candidate
   leaves the machine. Any ForgeVCS integration that bypasses it breaks House Rule 1 and the
   `FORGE_ALLOW_PUBLISH` semantics.
2. **Candidate identity** — a candidate is a **git commit** in the run workspace; QA freezes that SHA.
   ForgeVCS currently ingests **Scion changesets** (objects/trees), not git commits; there is **no Git
   import** yet.
3. **Workspace model** — Forge provisions a git/jj worktree; ForgeVCS has its own workspace service.
   Unifying these is architectural, not a flag.
4. **Authority** — Forge roles are service principals; ForgeVCS now has repo ACLs + human/service
   principals and a publish-only path. Integration must keep Forge's QA/DEV_OPS authority.
5. **Environment** — Forge is PROD-only; ForgeVCS has hosted + local/CI. A wiring change must be
   explicitly target-aware (`FORGE_VCS_TARGET=dev|prod`), never implicit.

## 3. Integration options (increasing ambition)

| Option | What | Pros | Cost / risk |
|---|---|---|---|
| **A. Read-only mirror** | After a successful `git_publish`, push the published `main` tree as Scion changesets into ForgeVCS for observability/audit. Forge remains authoritative VCS. | small, no authority change, proves the path | needs a git-tree→Scion-changeset converter (files/blobs/trees), repo mapping, auth principal |
| **B. Dual-write publish** | `git_publish` succeeds → mirror to ForgeVCS; a divergence alarm if the two drift. | catches drift, prepares cutover | converter + reconciliation + failure semantics |
| **C. ForgeVCS authoritative publish** | Replace the git push with: push candidate changeset to ForgeVCS, land/merge, verify ref; git becomes a mirror. | single VCS, merge/conflict workflow is canonical | requires git-commit→changeset import, candidate identity change, workspace alignment, rollback plan, HOLD semantics |

**Recommendation:** A first (read-only mirror, no authority change), then B with a drift alarm, then C
only after ForgeVCS hosted path/human path and the import converter are proven. Do not start with C.

## 4. DSH vs Forge — roles and artifacts

- **DSH (interactive, daytime):** the orchestrator/worker in a live session; can spawn agents and
  background jobs; artifacts must be written to files/rows, not held in chat. Best for coding,
  review, and coordination while you are present.
- **Forge (durable, overnight):** the workflow queue; owns claims/leases/attempts/QA/publish; best
  for 700-job background batches. It does not need an interactive chat; its truth is Neon.
- **The split:** DSH may *choose attention* and write work items through existing paths; Forge
  *executes deterministically*. Do not give DSH a second queue or let it hand-claim; use
  `orchestrate-status.sh` + the canonical HOLD/decision paths.
- **Artifacts:** anything DSH produces that must survive goes to `docs/agent/`, `forge_decision`, or
  the relevant evidence table — never only the transcript.

## 5. Gaps/risks confirmed

- No Git→Scion importer; no ForgeVCS Git wire protocol.
- No `FORGE_VCS_TARGET` concept; `git_publish` hardcodes git.
- ForgeVCS workspace model differs from Forge worktree.
- Hosted ForgeVCS unverified (local only); read-only mirror should target a non-prod ForgeVCS first.
- `git_publish` integration proofs are a safety property; any new publish path must preserve them.

## 6. Work orders

| WO | Scope | First acceptance |
|---|---|---|
| **WIRE-1** | Read-only mirror: published `main` tree → ForgeVCS repo changesets | **DONE** (2026-10-19): `scripts/forge-mirror.py` + harness; 6-path fixture, add/noop/mutate/noop; sha256 + exact tree id match; commit identity needs scion-import route |
| **WIRE-2** | Divergence check + alarm between git main and ForgeVCS mirror | drift detected with actionable evidence |
| **WIRE-3** | Candidate identity bridge: git candidate → Scion changeset (import converter) | QA-frozen SHA imports byte-identically |
| **WIRE-4** | Dual-write publish preserving `git_publish` proofs | publish fails closed if either target fails/drifts |
| **WIRE-5** | ForgeVCS-authoritative publish (only after 1–4) | rollback + HOLD semantics proven |

Start with WIRE-1; it needs no authority change and gives the first real answer to "does Forge's
published code survive a round-trip into ForgeVCS?"
