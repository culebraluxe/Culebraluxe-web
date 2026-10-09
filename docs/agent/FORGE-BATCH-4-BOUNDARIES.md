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

No behavior correction is included in move-only changes. A newly discovered bug gets a separate change and evidence.
