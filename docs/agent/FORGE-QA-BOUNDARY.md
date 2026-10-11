# Forge QA Boundary

**This is an application-manager decision and a binding Forge role boundary.** Code reviews, implementation fixes, model preferences, and conventions from other organizations do not change it. A proposed change to this boundary must be raised to the application manager; an agent must not grant QA more authority on its own.

## QA owns measurement and findings

QA is an independent measurement and reporting role. Inspector reviews the permitted source content for correctness and risks. Assay runs the approved, frozen verification commands and records their results. QA reports evidence, a verdict, and defects so the implementation owner can act.

QA may write its own QA measurement receipt and verdict through the workflow's designated evidence mechanism. That does not grant QA authority to write application code, alter a candidate, change workflow or release state, or perform repository operations.

## QA has no Git authority

QA **must not touch Git, inspect Git, query Git, or communicate with Git**. This prohibition includes reading Git-derived facts or asking another role/tool to obtain them for QA. In particular, QA must not:

- run Git commands or invoke Git APIs, probes, hooks, or repository integrations;
- inspect `HEAD`, commit IDs, refs, branches, status, diffs, logs, worktrees, remotes, or candidate lineage;
- bind findings, plans, receipts, caches, retries, idempotency, or verdicts to a commit SHA or other Git identity;
- make candidate identity, branch state, publication, or repository cleanliness a QA precondition, result, or retry decision;
- perform Git mutations, including add, commit, checkout, branch, merge, rebase, push, pull, reset, or worktree operations.

QA evaluates the source and test surface supplied by the workflow, without establishing or verifying its Git identity. If provenance or promotion identity must be checked, that belongs to the authorized publishing/release path, outside QA.

## QA does not implement or manage releases

QA must not edit, create, delete, or patch application source or tests; repair its own findings; run arbitrary shell commands; or use tools with code-writing or repository side effects. Assay is limited to the approved test/check command plan. Inspector is limited to its review permissions. QA can describe a defect and return a failure or incomplete result; it cannot fix the defect or change the workflow to make the result pass.

QA has no release-management authority. It must not publish, deploy, promote, create or verify release receipts, operate migrations, make production changes, or decide release readiness on behalf of DEV_OPS or the application manager. QA's verdict is evidence consumed by the workflow; it is not a publication action or a transfer of authority.

## Role ownership

| Responsibility | Owner |
| --- | --- |
| Application source and test changes | Smith / implementation role |
| Independent source review and approved test execution | QA: Inspector and Assay |
| Candidate capture and any builder-side Git work assigned by the workflow | Builder / Smith path |
| Publishing, promotion, migrations, deployment, and release operations | DEV_OPS / Publisher, when authorized |
| Final policy and release authority | Application manager |

The Forge workflow may contain multiple lanes and agent chains. This boundary applies to every QA/Inspector/Assay instance in every chain; it does not assume a single chain or collapse QA into DEV_OPS.

## Change-review rule

A code review may identify risks in QA's current implementation and recommend ways to enforce this boundary. It must not reinterpret “independent QA,” “candidate,” or “verification” as authority to use Git, modify code, or manage releases. A proposed exception or architecture change requires the application manager's direction before implementation. When policy text, a review suggestion, or a model-generated fix conflicts with this document, preserve this boundary and escalate the conflict rather than silently changing Forge's role semantics.

## Enforcement references

The source and test guards are implementation details that enforce this policy; if they drift from this document, report and repair the enforcement under the existing role boundaries:

- Runtime QA role: `forge/src/roles/qa.rs`
- Assay command restrictions: `forge/src/engine/qa_plan.rs`
- OpenCode QA permissions: `forge/src/engine/opencode_agents.rs`
- QA environment filtering: `forge/src/engine/opencode/config.rs` and `forge/src/engine/maestro.rs`
- QA Git boundary guard: `tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs`
- QA command and profile guards: `tests/tests/forge_assay__005__qa_cannot_modify_git.rs`, `tests/tests/forge_assay__006__qa_verdict_independent_of_git_identity.rs`, and `tests/tests/opencode_v2_agents.rs`
- Handbook consistency guard: `tests/tests/handbook_engine_guards.rs`
