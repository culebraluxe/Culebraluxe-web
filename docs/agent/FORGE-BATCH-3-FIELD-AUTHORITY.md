# Forge Batch 3 field authority matrix

This matrix describes the Forge evidence boundary after the Batch 3 typed marker change.
Role markers are proposals. The shared parser validates a node-scoped allowlist before merging any
field into durable workflow evidence. Any rejected field rejects the whole marker patch and names
the forbidden fields; the prior evidence remains intact.

The harness passes assistant text (not the provider transcript) to the marker decoder. It accepts
one marker only when it is the final nonempty, unquoted line and is outside Markdown code fences.
Duplicate markers, malformed JSON, wrong types, and unsupported versions are rejected. Schema 0
is a strict compatibility decoder for existing unversioned valid markers; new prompts require
schema 1. The accepted schema version and producer node, or the latest rejected marker's named reason, are
retained in durable workflow evidence. A rejected marker leaves prior business evidence unchanged while
recording the diagnostic for the correction path.

## Role marker fields

| Field | Owner | Marker rule |
| --- | --- | --- |
| `rootCauseKnown`, `diagnosisBlocked` | Scout | Scout nodes only |
| `researchDisposition` | Architect research node | `research_architect` only |
| `architectureSuspect`, `architectureReviewRequired` | Architect | Architect nodes only |
| `leadDecision`, `splitCount` | Lead decision node | `lead_pre` only |
| `failureClass`, `failedReleaseStage` | Lead failure classifier | `failure_classifier` only |
| `qaReviewPassed` | Inspector | Inspector nodes only |
| `qaPassed` | Deterministic Assay measurement | Rejected from every model marker |

## Specification, measurement, workflow, and release fields

| Evidence fields | Owner / authority |
| --- | --- |
| `workType`, `scoutRequired`, `qaReviewRequired`, `migrationRequired`, `migrationFiles`, `derivedRefreshRequired`, `derivedModels`, `deploymentRequired`, `deploymentDeferredToBatch` | Story Packet or operator planning boundary; role output cannot change the frozen requirement |
| `qaPassed`, `qaVerifiedSha` | Deterministic QA measurement and its candidate attestation |
| `candidateSha` | Authorized candidate-producing execution port |
| `publishSucceeded`, `devMigrationApplied`, `devMigrationVerified`, `prodMigrationApplied`, `prodMigrationVerified`, `derivedRefreshSucceeded`, `derivedRefreshVerified`, `deploymentSucceeded`, `deploymentDeferred`, `releaseDeferred`, `deploymentReceipt`, `productionVerified`, `productionVerificationReceipt`, `publishedSha`, `deployedSha`, `productionVerifiedSha`, `batchReleasedSha`, `batchReleasedAt`, `batchReleaseReceipt` | Authorized release, migration, refresh, deployment, or batch-release producer |
| `leadRouting`, `findings` | Structured role handoff ports; not accepted as marker fields |
| `disposition`, `resumeTarget`, `verificationGap`, `noProgress`, `repairAttempts`, `replanAttempts`, `lastFailure`, `classifierFailureClass`, `stageFailureClass` | Workflow or typed adjudication policy; not accepted as marker fields |
| `deliverableRejection` | Parser, lifecycle, or deterministic gate diagnostics |
| `roleOutputSchemaVersion` | Strict parser provenance |
| `extra` | Internal workflow facts; never a role marker escape hatch |

Unknown marker keys are rejected by the typed DTO, including fields listed above that require a
deterministic or authoritative producer. This matrix should be extended only when a concrete
producer port and its provenance are defined.

## Batch 1 coordination

The role-output authority boundary is independent of Batch 1's claim-fencing completion slices.
Assay receipt persistence and verdict application still need Batch 1 Slice 2's atomic completion
interface before they can be wired into one production lifecycle transaction. Batch 3 does not
modify Batch 1's completion, ledger, or control-plane files.

## Typed assay plan and approval contract

`forge/src/engine/qa_plan.rs` defines schema version 1. The Storyboard plan carries a stable
`plan_id` and positive `plan_version`, unique command IDs, explicit runner/parser pairs, command
strings, the currently supported `lane_root` working directory, an opaque executor environment
identity, unique check IDs bound to one command, and stable assertion references. Every acceptance
condition names checks and explicitly chooses `all_required` or `any_of`; omission defaults to
`product` judgment. RUST_CONTRACT plans also need a `test_artifact` condition. Optional negative
controls identify the approved command and the exact check IDs expected to fail.

The operator approval is the SHA-256 of the plan serialized through the typed schema. The
`forge-assay-plan-hash` binary enforces a 1 MiB input limit, validates the typed structure and
references, and prints the identity/hash an operator records beside the plan in Storyboard. It
reports packet-command binding as deferred; runtime checks exact binding against the authoritative
run snapshot. Approval metadata is not inferred from plan authorship. At Story Run insert, migration
281 copies plan and approval metadata into an immutable JSON snapshot. QA validates schema,
references, command equality with the run's frozen packet command snapshot, environment policy,
and approval hash before execution. The candidate must come from durable workflow evidence as a
40-character SHA; QA does not invoke Git to create lineage.

## Deterministic observation and blocker meanings

The production QA measurement nodes run without a model turn. They execute only the frozen plan
through the existing role execution port and persist the full receipt before returning a verdict.
Each check is parsed only from its own planned command output. Required checks default to all
required; `any_of` is accepted only when explicitly present in the approved plan. Zero-test Rust
success, skipped/ignored checks, missing or ambiguous assertion output, runner/parser mismatch,
and command substitution cannot prove PASS. Build failure, timeout, failed assertion, and missing
measurement remain separate observations. Product and test-artifact verdicts are recorded and
adjudicated separately.

Common receipt blockers include:

| Blocker | Meaning |
| --- | --- |
| `ASSAY_PLAN_REQUIRED` | No valid, approved plan snapshot is available; legacy prose and booleans cannot substitute. |
| `ASSAY_COMMAND_DRIFT` | Planned and observed command cardinality or identity differs. |
| `ASSAY_COMMAND_NOT_OBSERVED` | A planned command has no result. |
| `ASSAY_COMMAND_SUBSTITUTED` | A result at a planned position names a different command. |
| `ASSAY_COMMAND_UNPLANNED` | An observed result is outside the frozen plan. |
| `UNPROVEN <condition>` / `ASSERTION_NOT_PROVEN ...` | A required assertion is absent, skipped, ambiguous, unsupported, or otherwise not measurable. |
| `ASSERTION_FAILED <condition>` | The planned assertion was observed failing. |
| `CMD_BUILD_FAIL` | The command did not reach assertion measurement because the build failed. |
| `CMD_TIMEOUT` | The command hit the execution ceiling. |
| `NEGATIVE_CONTROL_UNPROVEN <command-id>` | The control did not observe the expected assertion failure, including when it failed for another reason. |

The receipt schema version 1 records story/run/process/task/node, plan and approval identity,
candidate SHA and source, command hashes and execution metadata, timestamps, bounded redacted
excerpts, parser version, check observations, both judgments, blockers, and truncation/redaction
flags. Its idempotency key scopes run, node, plan hash, and candidate. A same-key/different-content
write raises a conflict. Historical summary-only artifacts remain readable and are not promoted
into proof. The legacy TST worker has no frozen-plan input today, so it records observations as
`UNPROVEN` rather than granting PASS.

## Legacy conversion and rollout

Existing Storyboard rows and Story Runs remain NULL for the new plan fields. Convert a story only
when an operator can explicitly approve a typed plan and its hash. Do not synthesize checks from
acceptance prose or convert old `acceptance_mapped` booleans. Historical receipts remain available
for reporting but cannot certify a new run. Drain in-flight runs on the old snapshot contract
before deploying migration 281 and the new adjudicator; new runs without a plan visibly stop at
`ASSAY_PLAN_REQUIRED`. Migration 281 has been applied to Neon DEV for this branch's integration
verification; it has not been applied to production.

## Batch 1 integration and dependency points

Batch 1 Slice 2 now provides the atomic completion writer. This branch carries the assay receipt's
artifact ID, run, idempotency key, measurement node, verdict, plan hash, and candidate SHA through
the completion record. The DB transaction verifies that exact immutable artifact row before it
merges the verdict evidence and finalizes the completion receipt. The completion proof records the
assay receipt identity, and replay fingerprints include its idempotency key. A crash after the
assay artifact is saved but before task completion re-reads the same receipt without rerunning the
measurement, then retries the atomic completion unit.

Batch 1 Slices 3–4 are now in `origin/main` and this branch is rebased on that head. The rebase
preserved their claim-fenced completion/recovery behavior together with the assay-link validation
inside the final transaction. QA artifact writes stay outside the claim transaction: the complete
receipt is durable before its verdict is applied. The DEV integration test exercises the receipt
link through Batch 1 completion replay and provenance persistence against migration 281.

## Verification recorded for this branch

Verification on the branch: `cargo test -p forge --lib` (411 passed, 0 failed, 2 ignored),
`cargo test -p test-harness --test forge_runtime` (41 passed),
`cargo test -p test-harness --test forge_assay__007__pass_requires_acceptance_mapped` (1 passed),
`cargo check -p forge -p db`, the `forge_completion_receipt_dev` and `forge_assay_receipt_dev`
integration-test binaries compiled with `--no-run`, and the ignored `forge_assay_receipt_dev`
integration test ran against Neon DEV after migration 281 (1 passed). `pnpm forge:packet-lint`
(0 failures, 173 warnings, 157 baselined), `pnpm scripts:check` (54 shell files parse cleanly),
`pnpm scan:migrations` (0 findings), and `git diff --check` all passed. The DEV integration covers
receipt idempotency, content conflict, failed persistence, durable provenance, and Batch 1
completion replay/link. No production migration or deployment was performed.
