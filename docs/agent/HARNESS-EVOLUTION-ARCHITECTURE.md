# Harness Evolution — Architecture (native Rust)

**Status:** Batch A contracts landed 2026-10-18. Native Rust, no Python sidecar.
**Code:** `forge/src/engine/harness_evolution/model.rs` (contracts) and
`policy.rs` (protected surfaces + adoption policy).
**Reference:** [google-research/harness2](https://github.com/google-research/harness2)
(Apache-2.0). We adopt the *ideas* — the eight editable harness components, the
Propose–Probe–Compose loop, parallel/sequential recursion, and per-scope experience
libraries. No Python runtime, assets, benchmark harness, or source is copied or installed.

## 1. What this is

Forge already has a deterministic delivery machine (`FORGE_SDLC-v6.xml`, the seven role
services, OpenCode permissions, QA, rails) and a Phase 3 measurement layer (`forge scorecard`,
the daily digest, durable `forge_engine_task_execution` / `forge_hold_record` / QA rows).
Harness Evolution is the **bounded improvement layer above them**: it proposes harness edits,
probes them against the same task, contrasts behavior, composes a successor, runs Forge's
stronger validation, and adopts only through policy. The optimizer may improve guidance; it
may never grant itself authority.

```
Phase 3 signal (scorecard weakness, repeated correction, HOLD pattern, routing weakness,
skill/model-role weakness, cost/turn excess)
        |
        v
Propose              invariant candidates + domain candidates, kept separate
        |
        v
Policy pre-check     validate_candidate(): RED surfaces, authority, QA/Git, production,
        |            tool/MCP, workflow ownership, manifest exactness. Fails BEFORE probing.
        v
Probe                reference execution vs candidate execution, full identity pinned,
        |            never a production execution class
        v
Contrast             behavioral dimensions only (success, tests, QA, arch violations, tool
        |            failures, unnecessary calls, steps, tokens/cost, elapsed, repairs,
        |            escalations, holds, scope drift, forbidden attempts); identity mismatch
        |            invalidates the contrast
        v
Compose              per-component Keep/Adopt/Reject/Merge with evidence for/against;
        |            successor manifest built from the base, parent never overwritten;
        |            choosing the unchanged base is legal
        v
Forge Validate       static policy re-check + regression rails + replay validation +
        |            Phase 3 evidence review. A behavioral contrast alone is NEVER enough.
        v
Adopt / Reject       Green + clean -> automatic; Yellow -> Council; Red -> owner (Chris);
        |            incomplete -> more evidence; failed -> reject
        v
Reflect              store adopted harness + evidence-supported lessons in the
                     domain-scoped experience library
```

Every arrow is a typed value in `model.rs`; every refusal is a typed value
(`EvolutionError`, `PolicyViolation`), not a warning.

## 2. The eight components and authority classes

Harness² exposes eight editable components around the model. Our kinds and their authority
floors (`ComponentKind::minimum_authority`) are:

| Component | What it controls | Floor | Notes |
| --- | --- | --- | --- |
| `SystemPrompt` | the agent's instructions | GREEN | freely improvable, auto-adoptable |
| `Memory` | durable notes/context carried between turns | GREEN | clearing stale memory is legitimate |
| `Skills` | selectable skill bodies/pointers | YELLOW | capability surface; promotion controlled |
| `ToolGuidance` | how/when to use tools | YELLOW | guidance only; tool surface itself is red |
| `Subagents` | delegation prompts/config | YELLOW | arming/cost rules stay the harness's |
| `Guardrails` | safety, QA/Git and trust boundaries | YELLOW | emptying/removing is refused; owner-authored RED guardrails are owner-only |
| `Scripts` | executable scripts around the tool loop | RED | publication/DEV_OPS surface |
| `Plugins` | middleware/tool plugins | RED | tool/MCP surface expansion |

`AuthorityClass` is ordered GREEN < YELLOW < RED. A component's **effective** authority is
max(declared, kind floor) — a proposer cannot relabel a protected surface by claiming it is
green. Policy rejects any **authority expansion** and any **declassification** (declaring a
lower class than the base to unlock automatic adoption).

## 3. Protected RED surfaces

`policy.rs::ProtectedSurface` (owner-only; a candidate that touches one is rejected before
probing):

1. `authority_levels` — authority classes and permissions.
2. `qa_git_prohibition` — QA independence and QA's Git prohibition
   (`docs/agent/FORGE-QA-BOUNDARY.md`).
3. `production_permissions` — production write/deploy permissions.
4. `security_entitlements` — security and entitlement boundaries.
5. `publication_devops` — publication, release and DEV_OPS authority.
6. `workflow_xml_ownership` — ownership of `FORGE_SDLC-v6.xml` / the process definition.
7. `trusted_boundaries` — role/runtime trust boundaries.
8. `opencode_denies` — protected OpenCode deny rules.
9. `destructive_gates` — force-push/reset/drop-class destructive gates.

Detection has two independent layers so neither is a single point of trust:

* **Declared/structural:** RED declared authority, RED base revision, kind floors (Scripts,
  Plugins), Guardrails removal/emptying, authority expansion/declassification.
* **Content introduction:** `detect_protected_surfaces` scans changed content for protected
  topics. Only text **new relative to the base** is refused, so improving an ordinary prompt
  does not trip over an existing boundary sentence it does not touch.

## 4. Immutable revisions and atomic adoption

* A `Component` is a value; a `HarnessManifest` has **no mutation API** — construct a new one
  with `with_component` / `without_component` / `apply_changes`.
* A `HarnessRevision` carries a stable `manifest_hash`: SHA-256 over a domain-separated,
  sorted, length-prefixed canonical encoding (`MANIFEST_HASH_DOMAIN`). Insertion order does
  not matter; any content/authority/description/kind change does. `verify_integrity()` and
  `verify_against(stored_hash)` detect tampering.
* Candidate revisions are built only through `HarnessRevision::from_changes(base, id, changes,
  created_by)`, which makes `candidate.manifest = base.manifest + changes` structural. The
  policy re-checks exactly that equality.
* `RevisionStatus` is a closed lifecycle table: Draft -> Proposed -> ProbeReady -> Probing ->
  Compared -> Composed -> Validating -> Adoptable -> Adopted, plus Rejected/Failed/Superseded/
  RolledBack. `try_transition` refuses anything not in the table. Rollback restores the exact
  prior revision (Adopted <-> Superseded), it does not rebuild one.
* Active adoption is an atomic pointer swap in the Lead's catalog; experiments and failed
  probes can never mutate the active revision's manifest. The Lead's migration 294 stores
  revisions/candidates/probes/contrasts/compositions/experiences/activation; model rows are
  additive and reference existing Forge evidence ids, they do not copy telemetry.

## 5. Policy: what may happen, and when

**Pre-probe gate** — `validate_candidate(base, candidate, changes)` returns
`Result<(), Vec<PolicyViolation>>` and must pass before a `ProbeSpec` is created. It checks:
hash integrity, same domain, direct child (`parent_revision_id == base.id`), one change per
component kind, change shape (Set carries matching component, Remove carries none, rationale
required), RED/protected surfaces, authority expansion/declassification, Guardrails removal or
emptying, newly introduced protected-topic text, and manifest exactness. GREEN and YELLOW
changes are allowed here.

**Adoption policy** — `adoption_verdict(base, candidate, changes, validation)` classifies the
**effective** authority of every changed surface, not just the kind floor: per changed kind it
takes max(candidate declared, base declared, kind floor) and forces RED when the base revision
owns that kind as RED (`ChangedAuthority`). A failed validation gate -> `Reject`; incomplete
validation (a gate not run or evidence not linked) -> `MoreEvidence`; no effective change ->
`Reject`; RED -> `Chris`; YELLOW -> `Council`; every changed surface effectively GREEN + clean
-> `Adopt`. Thus a GREEN kind a candidate declares YELLOW never auto-adopts, and a
base-RED-owned component stays owner-only even when a candidate declares it lower. The base's
unchanged RED surfaces do not block an otherwise-green change.
`may_auto_adopt` is the explicit, tested automatic-adoption criterion; `changes_between`
reconstructs the canonical base→candidate change list for the gate (so the static policy
re-check proves manifest equality instead of checking an empty placeholder).
`adoption_recommendation` wraps this around `validate_candidate`, so a policy violation is an
unconditional `Reject`. `POLICY_VERSION` is carried on the recommendation for audit.

**Validation evidence** (`ValidationEvidence`) has four gates — static policy re-check,
regression rails, replay validation, Phase 3 evidence review — each `NotRun`/`Passed`/`Failed`.
Distinguishing "failed" from "not run" is what keeps the policy honest: a failed gate rejects,
an unfinished one asks for evidence, and a behavioral contrast alone is never sufficient.

## 6. Recursion budget and experience library

Harness² runs k reference executions of the base harness, then either:

* **parallel** — k invariant + k domain candidates probed, then k independent compositions
  from the shared evidence; or
* **sequential** — the refined harness and accumulated evidence carried through k
  Propose–Probe–Compose steps.

v1 is **k = 1** and does not implement parallel recursion. The contracts already express k > 1
(`EvolutionBudget{k,max_steps,max_probes}`, `CandidateSet.invariant/domain`, `EvolutionMode`)
so a later batch is a data/experiment change, not a model rewrite.

Experience libraries are separated by **domain, executor, mode, budget and seed**
(`ExperienceScope`). Tasks within a domain read only earlier entries
(`ExperienceRecord::is_visible_at`); domains may run concurrently. An `ExperienceRecord` must
carry a lesson, a behavioral contrast id, a **completed** `ValidationEvidence` (passed or
failed gates — both are lessons), evidence refs and applicability. An incomplete experiment
cannot update the library. Lessons expire (`expires_at`).

## 7. Relationship to Phase 3

Harness Evolution **consumes** Phase 3, it does not duplicate it:

* signals: `forge scorecard` weaknesses, the daily digest, HOLD patterns, repeated
  corrections, routing/model-role weaknesses, cost/turn excess;
* evidence: existing durable rows (execution, trajectory, holds, QA receipts, decisions) are
  referenced by id in `BehaviorEvidence.evidence_refs`; there is no new telemetry store;
* validation: existing rails, QA receipts and replay paths are gates, not new checks;
* no new queue: the Lead's experiment state machine runs on the existing Forge execution
  surface; adoption is a catalog pointer swap, not a second deploy path.

Constraints that stay: no fourth agent registry, no executive nodes added to
`FORGE_SDLC-v6.xml`, no write/publish authority for learned harnesses, no automatic authority
promotion.

## 8. Batch plan

| Batch | Owner | Scope | State |
| --- | --- | --- | --- |
| A | Architect | `model.rs` contracts, `policy.rs` protected surfaces + adoption policy, this doc | landed |
| A–B, E | Lead | `catalog.rs` (revision map, active pointer, atomic adopt/rollback), `probe.rs` (pinning/refusal/probe executor trait), `experiment.rs` (state machine, failure isolation), `adoption.rs`, migration 294 | next |
| C | Smith | `propose.rs`, `contrast.rs`, `compose.rs`, `validate.rs`, `experience.rs` | next |
| D | Lead + Architect | wire `mod.rs`, module exports, end-to-end test double (`RecordedProbe`) and one recorded experiment fixture | after B/C |
| E | QA | safety-rail verification, clippy, `pnpm forge:harness` | last |
| F (future) | — | k > 1 parallel/sequential recursion, routing metrics, correction rail | not v1 |

Wiring: `mod.rs` (Lead) declares `pub mod model; pub mod policy;` and the rest as batches
land; `engine/mod.rs` adds `pub mod harness_evolution;`.

## 9. Non-goals

* No Python sidecar, no `uv` install, no copied Harness² runtime or benchmark harness.
* No autonomous deployment/publication: adoption changes a harness pointer, not production.
* No self-granted authority: RED surfaces are owner-only by policy, not by convention.
* No new queue/telemetry/evidence store: Phase 3 rows and Forge receipts are the evidence.
* No "looks nicer" decisions: contrast is behavioral, and Forge validation is required.

## 10. Verification

```sh
cargo test -p forge harness_evolution::model
cargo test -p forge harness_evolution::policy
cargo check -p forge --all-targets
```

The contracts are covered by 46 unit tests: canonical hash stability and immutability,
serialization roundtrip, closed lifecycle transitions, candidate/revision construction,
budget/candidate-set k=1 defaults, probe identity matching plus production/same-revision
refusals, honest "not captured" dimensions, identity-mismatch contrast refusal, composition
evidence linkage, validation gate semantics, adoption authority rules, experience scoping, and
every protected-surface refusal (RED change, authority expansion/declassification, QA/Git,
production, workflow XML, OpenCode denies, destructive gates, tool/MCP, manifest mismatch,
parent/domain mismatch, duplicate changes).
