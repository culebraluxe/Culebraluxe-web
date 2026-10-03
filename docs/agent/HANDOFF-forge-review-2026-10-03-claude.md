# Handoff — Forge review and fixes, lane/claude, 2026-10-03

A review of the three days of Forge refactoring (2026-09-30 → 10-03), then fixes for what it found. Everything below is
pushed to `origin/main`; the lane is `lane/claude` and holds nothing unpushed.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | The trunk is `origin/main`; lanes rebase onto it and land by `git push origin lane/<name>:main` (fast-forward only). | `AGENTS.md` NO TREES → 2026-10-03 lane exception; `docs/agent/LAYOUT.md` § Lanes |
| S2 | The role services follow the `AbstractForgeService` model: the trait owns the turn sequence, each lane supplies hooks, the registry resolves by the XML service binding. `ProductionRoleRunner` is ports only. | `forge/src/roles/service.rs`, `forge/src/roles/hooks.rs`, `forge/src/engine/runner.rs` |
| S3 | Smith's candidate-acceptance rules now live in Smith (`judge_delivered_candidate`), applied through the new `ForgeRoleHooks::judge_output`; the OpenCode harness reports facts only. | `forge/src/roles/smith.rs`, `forge/src/engine/opencode.rs` (`CandidateProbe`) |
| S4 | `cargo test -p forge`: 209 pass. `cargo test -p test-harness`: 280 pass, 4 fail — the four are the known `forge_seam__001..004`. | the two commands |
| S5 | `cargo clippy --workspace --all-targets` has zero errors (warnings remain) and now runs in CI. | `.github/workflows/gates.yml` → "rust clippy" |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | `forge_seam__001..004` expectations | the workflow-lane owner (CURRENT.md) | Do not edit the expectations to pass. See §6.1 for what they actually trip on. |
| H2 | `.gitleaksignore 2` (147 fingerprints, committed in `5e05864a`) | the Captain | Do not delete or merge it; it is a security baseline question. |
| H3 | Applying PROD migrations from Forge | the Captain | Forge's migrate/refresh steps hold with a HUMAN STEP by design (`git_publish.rs`). Do not wire them to a pool. |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| A role's own behaviour | `forge/src/roles/hooks.rs` (the hook list) | `forge/src/roles/<lane>.rs` — never a `match node_id` in `lifecycle.rs` or the harness |
| How a failed run settles | `forge/src/bin/forge.rs` exit path; migration 263 `forge_settlement_pair` | `forge.rs`, `forge/src/engine/executor.rs` |
| Publishing a candidate | `forge/src/engine/git_publish.rs` `publish_candidate` | same; the scratch checkout is `worktree::with_detached_checkout` |
| Turn time limit | `FORGE_TURN_TIMEOUT_MINUTES` (default 120, `0`/`off` unbounded) | `forge/src/engine/opencode.rs` `turn_ceiling`, `opencode_client.rs` `max_turn` |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `1c95d719`, `9aa69348` | AGENTS.md lane exception to NO TREES; trunk is `origin/main` | `pnpm forge:packet-lint` 0 failures; `forge:sync-agents --check` ok |
| `a26ba686` | A failure hold settles `Error` (run `Failed`), not `Done`/`Complete` 100%; an engine fault in a role turn returns to the queue instead of a human hold | `forge_job__016` (new), seams 001/004/005/007, `forge_job__015`; forge 198/198 |
| `bd36fad0` | clippy's deny lint in `dead_commands.rs` | `cargo clippy -p cli --all-targets` 0 errors |
| `e88d4096` | Smith refactor: rules out of the transport into `SmithService`; Lead's solo implement judged by the same rules | `roles::smith::candidate_judgement_tests` (new, 5); forge 203/203 |
| `126f4f4d` | Model turns get a wall-clock ceiling (`TURN_TIMEOUT`); closed-stdout hang bounded too | 3 streaming tests against a real fake vendor + parser test |
| `46ed7902` | An integration (non-fast-forward) commit is proven with the story's QA commands in a scratch checkout before push; no proof or a failed proof = `IntegrationUnverified`, main untouched | `forge_publish__001` (new, 3, real bare remote); `forge_runtime` 41; `arch_boundary__011`; `repo_guards` 10 |
| `3a506f03` | Exit path classifies the typed error; bare `"timed out"` removed from engine-fault vocabulary | `engine_fault` rails; `forge_job__014`, `forge_arch_seam__007` |
| `6d2e9d1d` | Small debts (dropped settle error reported, unused params, doc dup); clippy step in CI | forge 207/207; YAML parsed |
| `ccffb289` | Migration/refresh refusals say HUMAN STEP instead of naming a dead TS host | forge 207/207; `forge_runtime` 41 |
| `3c433a32` | Lead regression from `792396d1`: Lead turns erased the standing `leadDecision`/`splitCount`; restored to the port's semantics | `roles::lead::tests` (new, 2); test-harness 280 pass / 4 known |

## 5. NOT VERIFIED — the honest gaps

- No live Forge run was made against PROD with these changes; all evidence is unit, L1 (in-memory engine) and L2 (real git) tests.
- The 120-minute turn ceiling is a judgement, not a measurement of the longest healthy Smith turn.
- Integration proofs run the story packet's QA commands in a fresh checkout with no `node_modules`; a story whose QA needs pnpm will hold `IntegrationUnverified` when main has moved. That is fail-closed by intent, not tested against a real TS story.
- One run of `forge_job__016` failed and then passed on rebuild with no code change in between; the shared `CARGO_TARGET_DIR` across lanes is the suspected cause, not proven.

## 6. OPEN — the next actions, in order

1. **The four seam failures.** They are fixture drift, not routing: the scripted harness sets `leadDecision` through chat JSON, which `lead_pre` refuses by design (the port did too), and the architect reply carries no marker `marker_evidence` reads as findings, so each role is self-healed once. Whoever owns the workflow lane: decide whether the fixture should supply the decision the way production does (bench intent / evidence), per CURRENT.md.
2. **Stale recovery can demote a `Complete` story.** `forge_hold_stale_work` (migration 266) sets the story `Hold` even when the item update changed no row. Needs a new migration (guard the story update on the item update's row count); a migration is a production change — Captain's go.
3. **One dispatch path.** `ForgeServiceRouter` and the direct runner path (`drive_forge_story` with `runner: Some`) survive only for fixtures; production uses the registry. Moving the fixtures to the durable driver would let both be deleted.
4. **Release evidence write.** `DbReleaseEvidenceStore::merge` reports a failed write only by `eprintln` (the DB failure itself is captured by `db::capture`); a lost `publishSucceeded` is re-derived on the next publish, which is idempotent. Make it return a result if that ever matters.

## 7. ASK THE OWNER

- Is 120 minutes the right default turn ceiling? (yes → nothing; no → change `DEFAULT_TURN_CEILING_MINUTES`.)
- Should the seam fixtures be updated to production's decision path (§6.1)? (yes → the workflow-lane agent rewrites the fixture; no → the four stay red and documented.)
- Go for the stale-recovery migration (§6.2)? (yes → write migration 268 and apply on DEV first.)
