# Forge 84 to 100 acceptance record

Implementation pass based on `Forge-84-to-100-Work-Order.md`, issued 2026-10-09. WO-07 is deferred at the
owner's request while the owner explores the plan-adoption process.

## Current evidence

- Lane started from `origin/main` at `8f1dd64e8`.
- Implementation commits: `3e4c48856`, `db5d62539`, `b421ef7f8`, and `1f5727265`.
- `cargo check -p db -p forge --all-targets`: PASS.
- `pnpm slice:check`: PASS. T0 49s, rustfmt 1s, T1 522s for `app-core`, `forge-engine`, and `harness`. T2 was not run.
- DEV integration: `forge_completion_receipt_dev__unit_is_one_transaction` PASS, including legacy-pending quarantine,
  explicit operator authorization, and atomic replay.
- DEV integration: `automatic_redispatch_keeps_one_budget_across_story_runs` PASS.
- DEV integration: `a_stale_engine_claim_is_recovered_once_and_a_fresh_one_never` PASS.
- Forge unit tests: final-heartbeat rejection, supervisor panic interruption, complete completion fingerprint coverage,
  canonical JSON object ordering, and engine-fault classification PASS.
- `pnpm db:parity`: PASS; zero table, column, index, foreign-key, or check drift.
- Read-only migration ledger/catalog inspection: migration 279 and 286 recorded on DEV and PROD with equal hashes.

| Migration | SHA-256 | DEV | PROD |
| --- | --- | --- | --- |
| `279_forge_stale_recovery_fencing.sql` | `bdd7984f3e039d0158123b1422ab9628b3fe61db865c20b261958cf8fbc0208c` | recorded | recorded |
| `286_forge_model_generation_budget.sql` | `0fbee338219c1bbdf63e057ae7e72cd8a46a9a3e708c834272dd892e7ff3c8ce` | applied | applied |

The production catalog query confirmed `forge_engine_task_execution.claim_generation`, the fenced
`forge_recover_stale_work(...)` routine, and both model-generation budget tables. Production and DEV were targeted
through the repository migration runner; `286` is additive and recorded through `schema_migration`.

## Work-order status

| Item | Status | Evidence / remaining proof |
| --- | --- | --- |
| WO-01 recovery schema and races | IN PROGRESS | 279 ledger/hash/catalog and stale-claim recovery test verified. A fresh disposable pre-279 migration rehearsal and final integrated recovery race evidence remain part of WO-08. |
| WO-02 completion authority | VERIFIED (code/test) | Final renewal failure rejects success; heartbeat supervisor panic interrupts the owned run and preserves an explicit authority error. Existing settlement owner/generation fence remains authoritative. |
| WO-03 legacy pending safety | VERIFIED (code/test) | Stale persisted pending receipts quarantine without effects; the operator procedure and explicitly authorized replay path are documented and integration-tested. |
| WO-04 durable assay replay | VERIFIED (code/test) | Saved receipt replay succeeds with candidate probing unavailable; new measurements still require candidate verification. T1 assay tests pass. |
| WO-05 full completion comparison | VERIFIED (code/test) | Versioned canonical SHA-256 fingerprint covers event identity, evidence, spend intent, and assay linkage; legacy finalized receipts fail closed without replay. T1 and fingerprint tests pass. |
| WO-06 cross-dispatch model budget | VERIFIED (code/test) | Stable work-item generation shares a frozen cap across Story Runs; DEV concurrency/redispatch test rejects excess attempts. Migration 286 conservatively saturates uncertain older generations. |
| WO-07 approved-plan adoption | OPEN — owner deferred | Not undertaken; requires the owner's plan-adoption exploration and approval authority. |
| WO-08 final release and integrated proof | IN PROGRESS | Code-level gates and database alignment pass. Final merged SHA, public web build SHA, resident worker build/config identity, and integrated post-release proof are not yet established. |

WO-02 through WO-06 are verified at the code/test level, not a claim that the unreleased resident worker has the
new behavior. The review's 100/100 acceptance is not complete until WO-08 release identities and integrated proof
are recorded; WO-07 remains open by the owner's direction.

## Operator reference

Legacy receipt inspection and resolution steps are in `docs/agent/FORGE-LEGACY-COMPLETION-RECONCILIATION.md`.
Logical generation, cap freezing, explicit fresh-generation behavior, and conservative migration policy are in
`docs/agent/FORGE-MODEL-GENERATION-BUDGET.md`.
