# TST RED landing — batches 10/11 landed, and the batches that are still stranded — 2026-10-08

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | The 20 canonical cases for DOCS.FORMS (14 stories) and DOCS.VAULT (6 stories) are on `main`, and all 20 pass | `git show --stat b4e2a791f`; `cargo test -p test-harness --test docs_forms_template__001__template_parser` |
| S2 | Those 20 stories read `Complete`/`100` with a real run row behind them — `b4e2a791f`, `1 passed`, `result_status='Complete'` | `select s.id, s.status, s.completion, r.result_status, r.commit_hash from storyboard_story s join storyboard_story_run r on r.story_id=s.id where r.commit_hash='b4e2a791f'` (PROD) |
| S3 | Three of the twenty had to be repaired before they would run at all: `docs_forms_template__007` did not compile (`fixed` missing from `TemplateFieldDefinition`), `docs_forms_template__001` asserted `newest(LISTING-01) == 4` while `middle/model/forms/templates/LISTING-01.v5.xml` is the head, `docs_forms_execution__007` froze a local `let execution_slot_id = …` that `web/src/api/portal_bridge/forms_write_actions.rs:548-549` builds inline as `executionSlotId` | the landing commit message; `middle/model/src/forms_format.rs:103-146` holds the production unit tests the money rule now mirrors |
| S4 | The frozen worktree-capability set is **6**, blessed by the Captain: `scripts/lane-cargo-config.sh` (matches as PROSE only) and `scripts/verify-checkout-build-dir.sh` (three disposable probes) | `cli/src/forge/repo_guards.rs:97-120`; `cargo test -p cli no_tree_residue` |
| S5 | **Only 6 of ~57 authored `TST RED batch` commits are on `main`** (batches 4, 6, 9, 17, 25, 33). Roughly thirty are stranded on lane branches with their stories stamped `Complete`: 5, 7, 8, 10, 11, 12, 15, 16, 18–23, 26–28, 30, 37, 39, 41, 43–45, 49–57 | `for c in $(git log --all --format='%H' --grep='TST RED batch' \| sort -u); do git merge-base --is-ancestor "$c" origin/main 2>/dev/null \|\| git log -1 --format='%h %s' "$c"; done` |
| S6 | `pnpm slice:check` is **red on `main`** for a reason this landing did not cause: `arch_boundary_011__qa_cannot_own_git_mutations` demands `pattern!(roles_that_may_not_commit, …)` in `cli/src/forge/lint.rs`, which no longer holds it | `/tmp/slice-receipt.txt` shape; `git show --stat b4e2a791f \| grep -cE 'lint\.rs\|arch_boundary__011'` = 0 — neither side of that assertion is this landing's |
| S7 | The fix for S6 already exists and is stranded: batch 28's own message says "fix stale `arch_boundary__011` assertion" | the S5 command, row `TST RED batch 28` (`3b32cc4ff`, `11f62276a` — the latter is `lane/longcat`'s copy) |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | TST-SEC-REDIRECT (batch 45): 5 of 10 never ran, 6 runs carry NULL evidence, stories read `Failed` | the Captain | Do not rework or re-stamp without his word; the seven authored cases are preserved, clean-applying, in `docs/agent/proposals/TST-REDIRECT-2026-10-08.patch` |
| H2 | TST-DOCS-VAULT-007 (`Complete`/`100`) and TST-FORGE-ASSAY-001..012 (`Complete`/`100`): their cases are authored in batch 12, which is not on `main` | the Captain | Do not mark these complete or rework them; landing batch 12 is a decision he has not made |
| H3 | The other ~25 stranded batches | the Captain | Do not bulk-land hundreds of authored files on a lane's own initiative: that is a story of its own, and §6.3 is its shape |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Land one more authored batch | §1 S5 for the sha; the batch's commit message names its stories | `git checkout <sha> -- 'tests/tests/<glob>'`, then `cargo test -p test-harness --test <stem>` per story |
| Record the receipt for a story | `docs/agent/HANDOFF-TST-REDIRECT-2026-10-08.md` (the blocked case) and §4 below for the shape that worked | `storyboard_story_run` (its view is `forge_story_run_receipt`); `storyboard_story.status/completion` |
| Add a file to the worktree-capability set | `cli/src/forge/repo_guards.rs`, the doc block above `WORKTREE_CAPABILITY_FILES` | that constant, **with the reason**, in the same commit |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `b4e2a791f` on `origin/main` | 20 canonical cases (`docs_forms_execution__001..007`, `docs_forms_template__001..007`, `docs_vault__001..006`), the harness helpers they need (`tests/src/source.rs`, +95), and `WORKTREE_CAPABILITY_FILES` 4 → 6 | `git push` exit 0; twenty assays `cargo test -p test-harness --test <stem>` — 20/20 exit 0, `1 passed` each; `cargo test -p cli no_tree_residue` exit 0; `pnpm slice:check --since 2c40d1fbb` → T0 PASS (26s), FMT PASS (2s), T1 FAIL on S6 alone |

The receipt row that worked, per story (`storyboard_story_run`):

```sql
INSERT INTO storyboard_story_run (story_id, started_at, ended_at, result_status, completion, commit_hash,
  base_commit_hash, tests_summary, run_type, execution_environment, run_phase, agent_runtime, notes,
  commands_total, commands_passed, commands_failed, tests_total, tests_passed, tests_failed,
  goal_snapshot, acceptance_criteria_snapshot, postconditions_snapshot, test_mode_snapshot,
  assay_commands_snapshot, packet_sha_snapshot)
SELECT :'story', :'started', :'ended', 'Complete', 100, :'sha', '2c40d1fbb', :'summary', 'assay', 'PROD',
  'post', 'cline:lane/deep', :'note', 1, 1, 0, 1, 1, 0, s.goal, s.acceptance_criteria, s.postconditions,
  s.test_mode, s.assay_commands, s.packet_sha
FROM storyboard_story s WHERE s.id = :'story';
```

`result_status` allows exactly `Complete|Partial|Blocked|Failed|Deferred|Hold|Cancelled|Interrupted` — a green assay is
`Complete`, **not** `Passed` (the check constraint `storyboard_story_run_result_status_check` refuses that word).

## 5. NOT VERIFIED — the honest gaps

- T2 (`cargo nextest run --workspace --profile ci`) was not run locally; it belongs to CI on `main`.
- The stranded batches' *contents* were listed, never executed: their files may be as stale as the three repaired here
  (one of which did not even compile), and no claim in this document says otherwise.
- Whether the ~380 other `Complete` TST stories have a case behind them was not re-measured; only the missing-receipt
  count from the session that raised this (398 of 577 `Complete` rows with no run) stands as reported.
- The `arch_boundary__011` verdict was inferred as pre-existing from the files it reads (this landing touches neither side).

## 6. OPEN — the next actions, in order

1. Fix S6 on its own slice: reconcile `cli/src/forge/lint.rs`'s commit-role rule with
   `tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:218-221`. Finished when `pnpm slice:check --since <base>` is green.
2. Land batch 12 (VAULT-007 + FORGE.ASSAY 001-009) if §7 answers "land batch 12", then receipt its rows by §4's shape.
3. Take the stranded set a batch at a time — never a bulk landing: the S5 command lists them, and each needs its own
   assay run before its rows are stamped, because a batch's own message claiming "10 green" has already been wrong here.

## 7. ASK THE OWNER

- **"land redirect"** → apply `docs/agent/proposals/TST-REDIRECT-2026-10-08.patch` (verified clean-applying), re-run its
  seven assays, record seven receipts and mark those seven rows `Complete`; 002/005/009 stay `Planned`.
  **"let failed stand"** → nothing happens; batch 45 keeps `Failed` and this row closes as unanswered.
- **"land batch 12"** → VAULT-007 + FORGE.ASSAY 001-009 land and are receipted like §4.
  **"no"** → they stay as they are, and VAULT-007's `Complete` stays unsupported.
- **"all batches"** → the stranded set of §1 S5 is worked batch by batch, assay first, receipt second.

## 8. AMENDMENT — Batch 45 verified against `main` (2026-10-08, later)

| # | Fact | How to check it |
| --- | --- | --- |
| A1 | All **11** `TST-SEC-REDIRECT-*` story rows read `Failed`/`completion=0`; only **6** carry a run row (`001, 002, 007, 008, 009, 011`), and every one of those is `result_status='Failed'` with an empty `commit_hash` and NULL `tests_total/tests_passed/commands_total/commands_passed`. `003, 004, 005, 006, 010` have no run row at all | `select s.id, s.status, r.result_status, r.commit_hash, r.tests_total from storyboard_story s left join storyboard_story_run r on r.story_id=s.id where s.id like '%SEC-REDIRECT%'` (PROD) |
| A2 | So the `Failed` verdicts have **no assay behind them** — "could not run" was recorded as "ran and failed" | A1: every run row's evidence columns are NULL |
| A3 | The patch re-applies clean against `877d1f6f6` (10 files), and **7/7 assays pass**: `sec_redirect__001/003/004/006/007/008/010` each `rc=0`, `1 passed; 0 failed`; `cargo test -p web google_auth` → `10 passed; 0 failed` | `git apply --check docs/agent/proposals/TST-REDIRECT-2026-10-08.patch`; then one `cargo test -p test-harness --test sec_redirect__<stem>` per case |
| A4 | The defect the patch repairs is **live on `main`**: `safe_next` (`web/src/api/google_auth.rs:77-81`) rejects `//` and `/\` but not control characters, and `callback` (line 224) pipes a percent-decoded `NEXT_COOKIE` straight into `Redirect::to`, so a `Location` value carrying CR/LF/NUL cannot become a header | `grep -n is_control web/src/api/google_auth.rs` → no match on `main` |
| A5 | Severity is low and it is **self-inflicted**, not cross-user: the attacker's own `callbackUrl` is the input, and the effect is a broken callback (500 / `rust:panic`) for whoever follows that link — no privilege change, no other user's cookie involved | the path in A4: `encode` at sign-in, `percent_decode` at callback |
| A6 | Coverage of the ten: `002`, `005`, `009` have **no case anywhere** (not in the patch, not on `main`); `011` has a story row and no case | `ls tests/tests/ \| grep sec_redirect` on `main` → empty; the patch authors 7 |

The verification run left the tree clean: the patch was applied, the eight test binaries were run, and the working
tree was reverted — so A3 is a measurement of the patch, not a state of the branch.

If §7's answer is **"land redirect"**: apply the patch, run the seven assays, write seven §4-shaped receipts
(`Complete`) for `001/003/004/006/007/008/010`, and set `002/005/009/011` to `Planned` (their `Failed` is A2's
unsupported verdict); the six evidence-free `Failed` run rows stay as they are unless the Captain says to remove them.
