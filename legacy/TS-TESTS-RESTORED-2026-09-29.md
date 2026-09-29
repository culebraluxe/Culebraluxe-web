# TypeScript test files restored from git — 2026-09-29

Source: the parent of `bad45d39` (2026-09-26, "delete 829 TypeScript files nothing runs,
builds or tests"), which deleted **465 test files** with the rest of the retired TypeScript estate.
Nothing runs these; they are reference, and the Rust ports in the 2026-09-29 HARDEN wave read them
as intent (`docs/agent/HANDOFF-ts-guards-to-rust-2026-09-29.md`).

- 384 restored at their original path (all under `legacy/`).
- 81 restored under `legacy/<original path>`: they lived in `agent-runtime/`, `testv2/` and `lib/` —
  the retired stack, which `scripts/ts-ratchet.sh` forbids in the product tree. The original path is
  the one listed below with `legacy/` removed.

Recover any deleted file directly:

```sh
git show bad45d39^:<original path>
```

## The 81 remapped paths

```
agent-runtime/accepted-candidate-publish-v6.test.ts
agent-runtime/accepted-candidate-publish.test.ts
agent-runtime/agent-runtime-adapter.test.ts
agent-runtime/assay-arithmetic.test.ts
agent-runtime/assay-evidence-regression.test.ts
agent-runtime/assay-evidence-v6.test.ts
agent-runtime/assay-human-intervention.test.ts
agent-runtime/assay-plan.test.ts
agent-runtime/candidate-assay-handoff.test.ts
agent-runtime/deterministic-assay-adapter.test.ts
agent-runtime/execution-contract.test.ts
agent-runtime/factory.test.ts
agent-runtime/forge-topology.test.ts
agent-runtime/forge-transition.test.ts
agent-runtime/gateway/cli-agent-adapter.test.ts
agent-runtime/gateway/provider.test.ts
agent-runtime/git-packet.test.ts
agent-runtime/harness-owned-commit.test.ts
agent-runtime/harness-usage.test.ts
agent-runtime/invoker-workspace.test.ts
agent-runtime/lane-policy.test.ts
agent-runtime/lead-decision.test.ts
agent-runtime/loop.test.ts
agent-runtime/opencode/forge-session.test.ts
agent-runtime/opencode/opencode-client.test.ts
agent-runtime/opencode/opencode-harness-adapter.test.ts
agent-runtime/opencode/opencode-routing.test.ts
agent-runtime/orchestrate-apply.test.ts
agent-runtime/orchestrate.test.ts
agent-runtime/readiness.test.ts
agent-runtime/recovery-policy.test.ts
agent-runtime/repo-context.test.ts
agent-runtime/repositories.assay.test.ts
agent-runtime/run-guardrails.test.ts
agent-runtime/run-machine-evidence.test.ts
agent-runtime/silent-failure-patterns.test.ts
agent-runtime/skills.test.ts
agent-runtime/slack-notifier.test.ts
agent-runtime/team.test.ts
agent-runtime/write-policy.test.ts
lib/worker-workspace/recovering-provisioner.test.ts
testv2/authorization.test.ts
testv2/calendar-landing-projection.test.ts
testv2/client-workspace-channel-projection.test.ts
testv2/client-workspace-controller.test.ts
testv2/composition.test.ts
testv2/contract-service.test.ts
testv2/engine_tests/baseline.test.ts
testv2/engine_tests/expressions.test.ts
testv2/engine_tests/hardening.test.ts
testv2/engine_tests/persistence/atomicity.test.ts
testv2/engine_tests/persistence/basic.test.ts
testv2/engine_tests/persistence/dynamic-fork.test.ts
testv2/engine_tests/persistence/human-task-lifecycle.test.ts
testv2/engine_tests/persistence/join-concurrency.test.ts
testv2/engine_tests/persistence/lease-reclaim.test.ts
testv2/engine_tests/persistence/retry-idempotency.test.ts
testv2/engine_tests/persistence/termination-races.test.ts
testv2/engine_tests/persistence/trace.test.ts
testv2/engine_tests/torture.test.ts
testv2/firm-service.test.ts
testv2/forms-template-registry.test.ts
testv2/person-service.test.ts
testv2/project-ownership.test.ts
testv2/project-service.test.ts
testv2/projects-assets-projection.test.ts
testv2/projects-catchup-projection.test.ts
testv2/projects-documents-projection.test.ts
testv2/projects-secondary-projection.test.ts
testv2/projects-service-projection.test.ts
testv2/projects-timeline-projection.test.ts
testv2/projects-workspace-01-readmodel.test.ts
testv2/projects-workspace-02-tokens.test.ts
testv2/property-service.test.ts
testv2/schema-parity.test.ts
testv2/security-service.test.ts
testv2/service-error-sink.test.ts
testv2/service-registry.test.ts
testv2/showing-service.test.ts
testv2/wbs-project-items-repository.test.ts
testv2/wbs-service.test.ts
```
