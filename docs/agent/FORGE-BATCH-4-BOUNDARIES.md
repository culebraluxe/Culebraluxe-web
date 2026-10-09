# Forge Batch 4: responsibility and API map

**Work order:** FORGE-B4  
**Baseline:** `0ba423ae0` (Batch 2 completion, rebased on `0e50ad07f`)  
**Purpose:** record the behavior/API contracts before extraction. This is an implementation map, not a new architecture.

## Baseline verification

- `cargo check -p test-harness --all-targets` — passed on the Batch 2 branch before creating this lane.
- `cargo test -p forge` — 436 library tests passed, 2 ignored; 1 Forge binary test passed.
- `cargo fmt --all -- --check` and `git diff --check` — passed.
- DEV-only database integration is not part of this baseline; it requires `DATABASE_URL_DEV` and migrations 282/283.

## Responsibility map

| Current file | Responsibilities currently combined | Existing boundary to preserve | Meaningful behavior checks |
|---|---|---|---|
| `forge/src/engine/maestro.rs` | Agent aliasing and roster preflight; CLI resolution; turn ceiling and continuity settings; session marker/vendor-session bridge; process launch, stream supervision and interruption; usage extraction; response parsing; `RoleHarness` and candidate/probe implementations; unit tests. | `RoleHarness`, `HarnessOutput`, `HarnessUsage`, `TurnTermination`, `vendor_session`, and the existing process/assay boundaries. | Inline Maestro tests; Forge harness/architecture tests; no vendor behavior change during extraction. |
| `forge/src/engine/opencode.rs` | CLI candidate resolution; model policy and environment sanitization; session continuity; `OpenCodeHarness`; assay environment/target directory; durable execution identity; harness tests. | Existing `opencode_client`, `opencode_events`, `opencode_agents`, `UsageBaseline`, shared lifecycle, `RoleHarness`, and execution control ports. | Inline OpenCode tests, `opencode_v2_contract`, `opencode_v2_agents`, harness usage tests, canonical execution-chain tests. |
| `forge/src/engine/job.rs` | Durable `JobService` implementation; job request/lease state; ownership fence and heartbeat; claimed-job execution; retry/deadline handling; targeted interrupt delivery; settlement and unit tests. | `JobService`, `WorkflowJobService`, the workflow store, execution-control port, shared role lifecycle, and Batch 1 claim/receipt guarantees. Vendor-specific configuration stays outside this generic layer. | Inline job tests plus `forge_job__*`, failure/chaos, and canonical execution-chain tests. |
| `db/src/forge_engine.rs` | `ForgeEngineDao` facade implementation and SQL/DTOs for agent-work claims/settlements, dispatch/reconciliation, packets/evidence, tool artifacts, model-attempt budgets, and completion receipts. | SQL and transaction ownership remain in `db`; `db::ForgeEngineDao` and the `db/src/lib.rs` re-exports remain the integration facade. | `tests/src/forge.rs`, `tests/src/race.rs`, Forge dispatch/claim/receipt tests, Batch 2 model-attempt checks. |
| `forge/src/engine/learn.rs` | Timestamp anchor; Git change discovery and file reads; JS-text heuristics over JS and Rust paths; stale-claim observation; candidate identity/ranking; open-key lookup; story/staging filing; one-story-per-pass policy; inline tests. | Existing `ForgeControlDao` service boundary, normal staged-learn flow, separate stale-claim path, human decision ownership, and no source/Git mutation by the learning pass. | Inline learn tests and the existing DEV filing/claim tests. |

## Public surface and consumers

- `forge::engine::job` is used by the Forge binary, independently compiled integration tests, and production-shaped test support. Keep the durable job facade public.
- `forge::engine::opencode` and `forge::engine::maestro` are used by the Forge binary and integration tests for explicit adapter construction, CLI/model resolution, contract checks, and harness tests. Narrow incidental internals only after consumer inventory; retain these supported entry points.
- `forge::engine::opencode_client`, `opencode_events`, and `opencode_agents` already divide transport, event, and agent concerns. Reuse these modules instead of duplicating them.
- `db::ForgeEngineDao` is re-exported from the private `db::forge_engine` implementation module. Preserve the DAO facade and public DTOs required by separately compiled tests and Forge.
- `forge::engine::learn` is private after Slice 3; only the in-crate worker calls its pass.
- `job_payload`, `model_aliases`, and `opencode_events` are private implementation modules after Slice 3. Source search found no external-crate consumers; the Forge binary and all integration targets compile against the retained facade.
- `forge::engine::mod.rs` still has other public declarations. Slice 3 narrows only the four consumer-verified modules above; it does not use visibility changes to hide architecture checks.

## Extraction sequence and protected behavior

1. Extract pure Maestro output/usage parsing into an internal child module; preserve parser errors and roster/output interpretation.
2. Extract cohesive OpenCode configuration/model helpers only where the existing facade keeps call sites stable; preserve candidate precedence, environment filtering, pinned model, and session behavior.
3. Split durable job mechanics by cohesive responsibility while retaining one `WorkflowJobService` and the existing job port. Preserve attempt count, deadline order, lease-loss interrupt, retry, settlement, and error distinctions.
4. Split DAO implementation by operation family with no SQL policy moved into Forge and no duplicate implementation.
5. Narrow incidental public modules after the consumer map, then proceed to the learning scanner and durable progress work in later slices.

## Slice 2 extraction record

- `forge/src/engine/maestro/output.rs` now owns the pure Maestro usage and response-envelope parser; the adapter facade still calls it and retains its parser tests.
- `forge/src/engine/opencode/config.rs` now owns CLI/model/environment/session configuration. The existing `forge::engine::opencode::*` entry points are explicitly re-exported, preserving binary and integration-test callers.
- `forge/src/engine/job/workflow_service.rs` now owns the Workflow-backed `JobService`; `forge::engine::job::WorkflowJobService` remains the same public construction point.
- `db/src/forge_engine/model_attempt_budget.rs` now owns generation budget SQL operations. DTOs and `ForgeEngineDao` remain at the existing db facade, and SQL has not moved out of `db`.
- No vendor launch behavior, job policy, SQL behavior, or public call path was intentionally changed.

## Slice 3 API record

- `learn` and `run_learn_pass` are no longer part of the external Forge API; the worker remains the sole caller.
- `job_payload`, `model_aliases`, and `opencode_events` are private implementation modules. Their remaining uses are inside `forge`.
- `cargo check -p test-harness --all-targets` passed after these visibility changes, covering independently compiled integration tests and the Forge binary.

## Slice 4 learning rules

- Rust and JavaScript/TypeScript scanning are separate modules. Rust rules parse syntax with `syn`; they do not infer types or execute source.
- Rust findings are review-only observations with a versioned rule ID, stable rule/path key, pinned source revision, span, normalized context, rationale, confidence, and stated limitation. The initial Rust catalog covers known-fallible `let _` calls, empty `Err` arms, and likely error-to-default fallbacks in authoritative paths.
- Rust observations are collected and retained, but normal filing is off by default. `FORGE_LEARN_RUST_RULES_ENABLED=1` is required to select them for filing; dry-run always evaluates the rules. This batch does not set that production flag. Existing JS/TS rules and stale-claim routing retain their current behavior.
- Narrow intentional cases cover discarded `remove_file`, `remove_dir_all`, and rollback results, plus the exact local anchor/session-marker read/write patterns whose absence is an expected fallback. The rules still cannot type-check receiver types, infer wrapper contracts, or see every generated/macro-expanded path; other safe `.ok()`/default fallbacks can be false positives, while APIs outside the fallible-call allowlist and indirect error suppression are known false negatives.
- Inline `#[cfg(test)]` modules and repository test/vendor/generated/build paths are excluded from production Rust rules. Rust comments and strings are handled by the syntax parser. Legacy JS/TS heuristics run only on JS/TS paths and retain their documented text-matching limitations.
- `forge learn --dry-run` is read-only: it does not connect to the control plane, file stories, or move a cursor. Its report includes the pinned revision, scanned/deferred file counts, bytes, findings, parser issues, and an incomplete status when scan bounds are reached.
- Bounds: 40 files and 4 MiB per pass, 256 KiB per source file, 8 MiB Git path output, 10 seconds per Git subprocess. A cap or parser/read error is visible and cannot advance durable progress.

## Slice 5 progress and filing

- Migration 284 adds `forge_learn_scan_state` (per stable repository identity, pinned cursor/revision, pending file paths, and pending observations) and `forge_learn_finding` (stable logical key, occurrence, and last story pointer). Durable scanning requires an `origin` remote so separate worker worktrees derive the same repository scope; only its sanitized, hashed identity is stored. The local `.forge-context/learn-last-run.json` remains a convenience cache only.
- Git change discovery is bounded and reports errors. Source bytes come from `git show <pinned-revision>:<path>`, never from a moving worktree. Deleted paths are excluded by Git's `ACMRT` filter because there is no current source to analyze; invalid paths, unreadable files, oversized files, invalid UTF-8, timeouts, and parser failures stop the pass visibly without cursor advancement.
- Progress updates compare the expected revision, file backlog, and observation backlog. Competing workers may read the same chunk, but only one compare-and-set advances it. Unselected findings stay in the JSON backlog; only one story is filed per pass, and the worker logs deferred file/finding counts.
- `ForgeControlDao::file_learn_finding` serializes a logical key and commits the registry pointer with either the existing Ready dispatch or the staging batch item. Retrying after a crash returns the still-open story. A resolved/staged/terminal story is not a permanent tombstone: a later recurrence increments the occurrence and receives a new deterministic story ID.
- The stable identity is `rule ID + rule version + repository-relative path`, paired with a hash of the sanitized origin identity. Line numbers and filing time do not change the key. Existing pre-migration open keys are checked as a compatibility fallback.
- Stale claims remain observations only and use their existing Ready route; the learning pass never recovers a claim. No migration was applied to DEV or production as part of this lane.

No behavior correction is included in move-only changes. A newly discovered bug gets a separate change and evidence.
