# ENG-FORGE-V4-10B — Accepted Candidate Publish

## Goal
Make Forge behave like a CMS after a clean Assay: accepted Forge code should publish to `origin/main` automatically so Chris does not have to manage branches, pushes, or merges by hand.

## Scope
Add one small publish step after a clean Assay result for a Smith-built candidate.

- Publish only after Assay is clean and the story is otherwise eligible to complete.
- Require a real Smith candidate commit; never publish an empty/no-change candidate.
- Publish directly to `origin/main` only when it is a safe fast-forward from the candidate's recorded base/current remote main.
- Never force-push `main`.
- If remote `main` advanced or the candidate cannot be fast-forwarded safely, preserve the candidate commit and fail closed into a clear Hold/publish-conflict state rather than losing work or rewriting history.
- The publish helper runs in the outer Forge process, not inside the model sandbox.
- Keep Git mechanics invisible to the operator during the normal happy path.
- No schema change, no provider change, no swarm/parallel behavior.

## Architect brief
V4-09 made the outer Forge harness create candidate commits. The remaining gap is publication: a story can be Complete in Neon while its accepted code still exists only inside a local worktree. Close that gap.

Treat GitHub/main as the deployment source for Forge code, but Neon remains the run/story system of record. Publication is a post-Assay acceptance action, not part of Smith execution and not something Assay itself may perform.

The safe happy path is intentionally boring:

Smith edits/tests -> outer Forge candidate commit -> Assay clean -> verify remote main has not diverged -> push candidate commit to origin/main -> mark/preserve evidence -> story Complete.

Do not add PR ceremony for this isolated Forge path. Do not use `--force`. If safe direct publication is impossible, stop and retain the candidate for repair/retry.

## Acceptance criteria
1. Clean Assay + valid Smith candidate + unchanged compatible `origin/main` publishes the candidate to `main` automatically.
2. No candidate commit means no publication.
3. Failed/Hold Assay never publishes.
4. Remote-main divergence never force-pushes and never discards the candidate; return a factual publish conflict and leave story in Hold/repair rather than Complete.
5. Publication happens from the outer Forge harness, never from the model sandbox.
6. Candidate commit hash and published main hash are reported in operator/runtime evidence where practical without schema changes.
7. Existing V3/V4 lane ordering, execution-contract, harness-owned commit, readiness, and Assay semantics remain unchanged.
8. Focused tests cover happy-path publish, no-candidate, failed-Assay suppression, and diverged-main fail-closed behavior.

## Test mode
Scoped only. Do not run the full regression suite.

## Assay commands
- `pnpm exec tsx --test agent-runtime/accepted-candidate-publish.test.ts`
- `pnpm exec tsx --test agent-runtime/harness-owned-commit.test.ts agent-runtime/readiness.test.ts agent-runtime/gateway/cli-agent-adapter.test.ts agent-runtime/gateway/provider.test.ts agent-runtime/team.test.ts agent-runtime/orchestrate.test.ts agent-runtime/orchestrate-apply.test.ts agent-runtime/repositories.assay.test.ts`

## Skills
- TypeScript
- Git plumbing / fast-forward safety
- child-process execution
- Forge lifecycle and Assay semantics
- focused test seams

## Loop
Architect packet -> Smith -> candidate commit -> Assay -> outer Forge safe publish to main. No PR ceremony, no force push, no swarm, no schema.
