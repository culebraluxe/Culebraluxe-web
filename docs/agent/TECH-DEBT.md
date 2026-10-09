# TECH DEBT

Work the captain explicitly paused or that must be circled back on lives in `docs/agent/OPEN-QA-LIST.md`
(created 2026-09-28) — that is the list he asked for, and it is the first place to look for "what is
finished but not yet proven". This file stays what it was: known-wrong things.

What we know is not right, recorded so it is not lost and not re-discovered. Three rules:

1. **Blocking debt is at the top and gets fixed before ship.** Everything below it waits its turn.
2. **Debt has a name, a place and an exit.** "Later" is not an exit; "delete this line when X exists" is.
3. **Baselines are recorded debt.** The harness-lint baseline (`docs/agent/harness-lint-baseline.json`)
   holds findings we chose not to fix on day one. It is **empty** as of 2026-09-15, the same day it was
   recorded — both of its debts were paid rather than carried, and the file says so in prose.

Last reviewed: **2026-10-08 evening** (`lane/deep`): the six red targets on trunk — `arch_boundary__011`,
`forge_arch_seam__001`, `prop_property_based__008`, `wf_token__006`, `wf_token__007`, `runtime_deploy__004` —
reproduced on `b3be12ec6`, each given an owner and an expiry, and the SEC.REDIRECT id collision and the in-checkout
build tree recorded while doing it. The rustfmt drift on trunk closed in the same pass (`6fe65a1a3`). **Six of those
rows have closed since**: 3, 6, 8 and 9 (`1c55a567b`) by the captain's calls ("exclude tests", "land 008", "assign
8 9"), then 4 and 10 (`0be8e4949`, `e83da7256`) on his one word **"FIX"** — row 7's code half travelled with row 10,
being the same file. So **two red targets remain, and both are a decision rather than a repair**:
`arch_boundary__011` (row 1 — the assertion whose own message says a QA module needing that name "is an architecture
decision, not a test edit"; rows 3 and 4 sat behind it and are closed, so it stands alone in that guard) and
`forge_arch_seam__001` (row 2, the job layer reading `opencode::turn_ceiling`). Each wants one word from the captain
— **WIDEN** the detector or **MOVE** the call — and no lane may decide it for him. The previous review was 2026-09-15
(the
debt-clearing pass: release-path bundler,
the V9 live throw, `.next` duplicates, the empty lint baseline, KIND chips, and the harness wired into the release
build) — that pass's estate is the retired TypeScript one, kept below under "Superseded".

## Blocking — the red targets on trunk, assigned 2026-10-08

`main` carries **six red test targets**. Each was reproduced before it was assigned, not read off a hand-off: rows 1
and 2 by running the target on its own, rows 6, 8, 9 and 10 by the trunk estate run (10 also by hand, with the
child's environment genuinely cleared), and the receipts are at the foot of this section. Rows 3, 4, 5 and 7 are not
red targets — they are what putting the trunk back in order turned up, and they are recorded here because each needs
an owner for the same reason. Every row names a **place**, an **owner** and an **expiry** — assigned on the
captain's direction 2026-10-08, because a red row that nobody owns is what stops every lane's T1
(`pnpm slice:check` runs the section, and the section stops at the first red target).

| # | Debt | Place it is decided in | Owner | Expiry | Exit that closes it |
| --- | --- | --- | --- | --- | --- |
| 1 | **`arch_boundary__011` refuses `forge/src/engine/assay.rs` naming `Command::new`.** The QA surface owns no process spawn; FIX-005 added one to bound every wait (`spawn_scoped_shell`, `spawn_scoped_shell_with_env`: `assay.rs:203`, `:232`; `:764` in its `#[cfg(test)]` module). | Assert `tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:632`; name arrived in `bd1b093f5` (FORGE-FIX-005, merged `530903627`) | **lane/muse** — author of FIX-005 | 2026-10-10 | `cargo test -p test-harness --test arch_boundary__011__qa_cannot_own_git_mutations` green |
| 2 | **`forge_arch_seam__001` refuses `forge/src/engine/job.rs` naming `opencode`** — the job layer reaches the vendor to read a turn ceiling (`job.rs:542-544`, `crate::engine::opencode::turn_ceiling`). | Assert `tests/tests/forge_arch_seam__001__canonical_execution_chain.rs:281`; name arrived in `32832c57c` (FORGE-FIX-006, merged `1a0f60269`) | **lane/nemotron-2** — author of FIX-006 | 2026-10-10 | `cargo test -p test-harness --test forge_arch_seam__001__canonical_execution_chain` green |
| 3 | **Behind #1 — the mutation-verb sweep counts a test file.** The sweep requires that only `forge/src/engine/git_publish.rs` names `"push"`/`"merge"`/`"rebase"`; measured, exactly three files in the workspace name such a literal — `git_publish.rs` (allowed), this guard (excluded by `SELF`), and `tests/tests/forge_assay__005__qa_cannot_modify_git.rs:32-34`, a test whose subject *is* that QA may not push. | Assert `arch_boundary__011…rs:880`, `workspace_code()` (`:395-405`) sweeps `tests/**` | **lane/deep** | 2026-10-10 | **CLOSED 2026-10-08 (`1c55a567b`)** — `TEST_TREE = "tests/"` is that one named, dated line: the sweep reads the production tree only, where exactly one file names a verb (`forge/src/engine/git_publish.rs`, once — the guard's own `Some(1)`), and section 5 is green *observed*, by a run that reached section 6 (`:924`). Receipt at the foot |
| 4 | **Behind #1 — the lint's `pattern!` spelling.** `ROLES_THAT_MAY_NOT_COMMIT` (`arch_boundary__011…rs:220-221`) pins the *one-line* spelling of the lint's role pattern; `cli/src/forge/lint.rs:105-108` now spells it wrapped across three lines, so `lint.contains(…)` cannot match. | Assert `arch_boundary__011…rs:924`; `cli/src/forge/lint.rs:105-108`, wrapped by the rustfmt pass `430a13761` | **lane/deep** — landed the fix `lane/longcat` had written but not pushed (`3b32cc4ff`, batch 28) | 2026-10-10 | **CLOSED 2026-10-08 (`0be8e4949`)** — the assertion names the regex the lint must name (`r"(?i)\b(scout|assay|inspector)\b"`, `arch_boundary__011…rs:233`, asserted `:927`), not the macro's line breaks, and it was **observed**: with row 1's assertion neutralised for one run (temporary, reverted the same session, `git status` clean) the guard is **green** — `1 passed; 0 failed`, `EXIT=0` — so sections 5 and 6 both pass on trunk and row 1 stands alone in that guard. Receipt at the foot |
| 5 | **Six SEC.REDIRECT story ids carry two test files each.** Three batches authored tests for the same stories and nothing says which file answers a story id: `b3ce7fef9` (2026-10-07, batch 45 — its own subject: "blocked by web crate compilation error") added `001__path_traversal`, `002__open_redirect`, `003__host_injection`, `004__percent_encoding`, `005__fragment_handling`, `006__query_parameters`; `68ba004de` (2026-10-08) added the ids' canonical files as `docs/agent/proposals/TST-REDIRECT-2026-10-08.patch` names them (`001__relative_path_allowed`, `003__evil`, `004__evil`, `006__userinfo_tricks`, and the single `007`/`008`/`010` files); `f438d7a68` (2026-10-08) added `002__same_origin_absolute_behavior_as_intended`, `005__encoded_slash_backslash_variants`, `009__malformed_unicode`, `011__redirect_loops`. | `ls tests/tests/sec_redirect__*.rs` — 17 files, ids 001–006 doubled | **lane/muse** — author of all three batches (every one of the three commits arrived on `lane/muse`) | 2026-10-10 | One file per id: each doubled pair folds into the canonical file — batch 45's assertions move inside it — and batch 45's name is deleted, so the id-to-file map is a function again. Naming the canonical half is done (that is `docs/agent/HANDOFF-TST-REDIRECT-2026-10-08.md`); the fold is not |
| 6 | **`prop_property_based__008__monetary_arithmetic` is red on trunk, and the stale half is the test.** It asserts `format_money("1250000") == "$1,250,000"` and `"1250000.5" == "$1,250,000.5"`; `middle/model/src/forms_format.rs:12` was rewritten for forms v5 by `c483e8f81` to always write two decimals (`"$1,250,000.00"`, `"$1,250,000.50"`), and the **passing** test `tests/tests/docs_forms_template__007__field_formatting.rs:42-43` pins that two-decimal contract. Found by the trunk estate run 2026-10-08: `test prop_property_based_008__monetary_arithmetic ... FAILED`, `left: "$1,250,000.00"`, `right: "$1,250,000"`. The formatter is the canonical half — its own doc says two decimals and a second test agrees. | Assert `tests/tests/prop_property_based__008__monetary_arithmetic.rs:76-77`; batch 36 (`17bf934a6`, authored against a tree without `c483e8f81`) | **lane/muse-2** — batch 36's lane | 2026-10-10 | **CLOSED 2026-10-08 (`1c55a567b`)** — those two assertions take the v5 contract, **and so does the display property behind them**, which was failing on `amount = "0000"` (0 commas, 1 expected) and had been masked by the two ahead of it: grouping is now measured on the *rendered* whole part and the output must carry exactly two decimals, which is stronger and true (`"0000"` → `"$0.00"`, `999.999` → `$1,000.00`). `cargo test -p test-harness --test prop_property_based__008__monetary_arithmetic` → ok, 1 passed. The hostile-input assertions are untouched, and the new failing seed is committed |
| 7 | **A test builds the whole workspace inside the checkout.** `runtime_deploy__004__server_executable.rs:45` falls back to `<checkout>/build/rust` when `CARGO_TARGET_DIR` is unset — the shared target dir retired 2026-10-07 — so running it does a **release build inside the tree**, at a path no `.gitignore` covers (`/target/` is ignored, `build/` is not). Measured 2026-10-08: the directory appeared mid-run and `git add -A` died on `build/rust/release/deps/rmeta…/full.rmeta`, a file the build was still rewriting. Two guards also describe `build/rust` as the shared dir in their doc comments, so the path reads as current. | `tests/tests/runtime_deploy__004__server_executable.rs:44-52` (batch 41, `ebed4727a`); the only test with that fallback (`grep -rl 'join("build")' tests/tests/` → 1) | **lane/muse** — batch 41's lane | 2026-10-10 | **Code half landed 2026-10-08 (`e83da7256`, lane/deep)** — `cargo_target_dir` (`runtime_deploy__004…rs:99-119`) asks cargo instead of guessing: `CARGO_TARGET_DIR`, then `cargo metadata`'s `target_directory` (which honours each lane's `.cargo/config.toml`), then cargo's own default `<root>/target`; the `CARGO_MANIFEST_DIR` fallback that named `lane-nemotron` is compile-time now. Measured after the target ran twice: `ls -d build` → **No such file or directory**, `git status --porcelain` → empty. **Still open: the doc-comment half** — five guards (`arch_boundary__012…:115`, `arch_one_writer__002…:64,210`, `_003…:54,157`, `_004…:49,145`, `arch_route_map__006…:59`) still describe `build/rust` as shared by every lane. That prose is lane/muse's row; lane/deep did not write in another lane's guards |
| 8 | **`wf_token__006__optional_branch_cannot_prevent_completion` — the process stays `Active` after every required token completes.** In-memory (`EngineHarness`), so it is red in CI too and not a database artefact; the run that found it (2026-10-08) is the first that ever got this far, because the rust-format step above the test step in `gates.yml` was red until `6fe65a1a3`. Which half is stale is a **workflow-engine call**, not a test edit: the test pins "completion is gated on required tokens only". | Assert `tests/tests/wf_token__006__optional_branch_cannot_prevent_completion.rs:271`; batch 60 (`5fdfbf4ae`, 2026-10-08) | **lane/deep** — moved from lane/muse by the captain 2026-10-08 ("assign 8 9"); the `middle/workflow` half needed no change | 2026-10-10 | **CLOSED 2026-10-08 (`1c55a567b`)** — the ruling is that the **engine is right and the case was stale** (see the workflow ruling below). Completion is `count_active_tokens == 0` (`execute_node_leave.rs:173-202`); an optional branch is retired at the **join** (`handle_join.rs:42-61`), which is the shape `RE_supermodel-v1.xml:199-204` relies on. The graph now has that join, its retired sibling lands `Completed`/`Skipped` with its task obsoleted, the process completes, and the retired task refuses completion. Target green |
| 9 | **`wf_token__007__required_branch_does_prevent_completion` — a cancelled process's tokens carry `TokenOutcome::Completed`.** The instance does reach `Aborted`; the tokens do not carry `Cancelled` (`left: Some(Completed)`). Also in-memory and CI-visible. Same rule question as row 8, opposite direction: whether a mass cancel must write its own outcome or may reuse the completion path is the engine's decision, and the test asserts an implementation detail it also documents as one ("`complete_token` is called with `Cancelled` outcome"). | Assert `tests/tests/wf_token__007__required_branch_does_prevent_completion.rs:332`; batch 60 (`5fdfbf4ae`) | **lane/deep** — moved from lane/muse by the captain 2026-10-08 ("assign 8 9"); the `middle/workflow` half needed no change | 2026-10-10 | **CLOSED 2026-10-08 (`1c55a567b`)** — cancellation already writes its own outcome: `terminate_process` marks every token it catches active `Cancelled` (`execute_node_leave.rs:223-238`). The assertion was over-broad — it read **every** token of the instance, including the fork's own parent, which `handle_fork` had concluded `Completed` before the cancel (`:385`) and which cancel must not rewrite. It now asserts the caught tokens carry `Cancelled`, that every token is concluded either way, that the one pre-concluded token is the fork's parent keeping `Completed`, and that no task is left open. Target green |
| 10 | **`runtime_deploy__004__server_executable` fails on its own environment handling, not on the server.** Its negative case spawns the built `web` with `PORT` set and `APP_ENV` deliberately omitted — but it does not *clear* the child's environment, so it inherits whatever the test process has: run with `.env.local` sourced (every lane) the child starts and serves, and the test fails. Measured 2026-10-08 with the environment actually cleared (`env -i PATH=… PORT=8099 ./build/rust/release/web`) the binary exits with `DbFailure … "database target is undeclared; set APP_ENV or use VERCEL_ENV"` — the refusal is real, and the child is not given the conditions the case needs. The 500 ms wait before `try_wait` adds a second flake. | `tests/tests/runtime_deploy__004__server_executable.rs` — the four negative cases (batch 41, `ebed4727a`); the child env is set at `:71-80`, the waits at `:43-62` | **lane/deep** — batch 41's lane left the case as it found it; the fix landed here | 2026-10-10 | **CLOSED 2026-10-08 (`e83da7256`)** — `refusable_command` (`:71-80`) clears the child's environment and keeps only `PATH`, so "APP_ENV intentionally not set" is true of the process the server sees; `wait_for_exit` (`:43-62`) polls on a bound (30 s to refuse, 5 s for SIGTERM) instead of sleeping 500 ms. Green both ways: `.env.local` sourced → `1 passed; 0 failed … finished in 0.68s`, `EXIT=0` (the run that used to fail) and under `env -i PATH=… HOME=…` → `1 passed; 0 failed … finished in 0.29s`, `EXIT=0`. Receipt at the foot |

Rows 3 and 4 sat *behind* row 1 in execution order — the guard stops at `:644`, so neither was observable in an
ordinary run: a `grep` for the three literals names exactly the three files above, and a fixed-string search for the
one-line `pattern!(roles_that_may_not_commit, …)` in `cli/src/forge/lint.rs` returns **0** while the regex it names is
present (**1**). Both were *observed* the same way — with row 1's assertion neutralised for one run, a temporary edit
reverted in the same session because row 1 is lane/muse's to fix, and `git status` shown clean after it. The first run
reached **section 6** and stopped at `:924`, which was row 4; after `0be8e4949` the same run is **green**
(`1 passed; 0 failed`), so sections 5 and 6 are both satisfied on trunk and **row 1 is the only thing still standing
in that guard.**

**The workflow ruling (rows 8 and 9), because it took evidence and not a preference.** The engine's completion rule is
one line and it counts **every** token: `check_process_completion` completes the instance only when
`count_active_tokens == 0` (`middle/workflow/src/engine/execute_node_leave.rs:173-202`; both stores count
`status = 'active'`). A `required: false` branch cannot block completion **because the join retires it**: `handle_join`
waits on required siblings only (`handle_join.rs:42-44`), concludes each still-active optional sibling
`Completed`/`Skipped`, obsoletes its task and cancels its job (`:46-80`) — which is what the production definition says
in its own words ("they skip straight to the join … and are skipped by the join (timer cancelled)",
`forge/definitions/RE_supermodel-v1.xml:199-204`) and what the two green cases of the same batch assert
(`wf_join__002`, `wf_definition__011`). A fork whose branches converge on an `end` node with **no join** is not a shape
this engine has a rule for, and nothing retires a token there — so 006's graph, not the engine, was the stale half.
Counting required tokens only would have left an Active token and an open task inside a Completed instance, which is
what `wf_join__006` exists to forbid. **No production line changed:** both repairs are in the test files.

Receipts (run 2026-10-08 on `b3be12ec6`, lane/deep):

    $ cargo test -p test-harness --test arch_boundary__011__qa_cannot_own_git_mutations
    thread 'arch_boundary_011__qa_cannot_own_git_mutations' panicked at
    tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:632:13:
    forge/src/engine/assay.rs names `Command::new`. The QA surface owns no git door: …
    test result: FAILED. 0 passed; 1 failed; 0 ignored                              EXIT=101

    $ cargo test -p test-harness --test forge_arch_seam__001__canonical_execution_chain
    thread 'the_job_layer_and_registry_know_no_role_no_node_and_no_vendor' panicked at
    tests/tests/forge_arch_seam__001__canonical_execution_chain.rs:281:5:
    JobService / the registry branch on a role or reach the vendor:
    forge/src/engine/job.rs: `opencode`
    test result: FAILED. 0 passed; 1 failed; 0 ignored                              EXIT=101

    $ grep -rlE '"(push|merge|rebase)"' --include='*.rs' forge middle db web cli ui tests
    forge/src/engine/git_publish.rs
    tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs
    tests/tests/forge_assay__005__qa_cannot_modify_git.rs

    $ grep -c 'pattern!(roles_that_may_not_commit, r"(?i)\b(scout|assay|inspector)\b");' cli/src/forge/lint.rs
    0

Second pass the same day (`1c55a567b`, lane/deep) — the four rows it closed, and what is left behind row 1:

    $ cargo test -p test-harness --test prop_property_based__008__monetary_arithmetic \
        --test wf_token__006__optional_branch_cannot_prevent_completion \
        --test wf_token__007__required_branch_does_prevent_completion --no-fail-fast
    test result: ok. 1 passed; 0 failed   prop_property_based__008__monetary_arithmetic
    test result: ok. 1 passed; 0 failed   wf_token__006__optional_branch_cannot_prevent_completion
    test result: ok. 1 passed; 0 failed   wf_token__007__required_branch_does_prevent_completion   EXIT=0
    (007's single run over its own case: "exactly one token — the fork's own — was concluded before the
     cancellation", i.e. the pre-concluded token is real and named, not an assumption)

    $ cargo test -p test-harness --test arch_boundary__011__qa_cannot_own_git_mutations
    panicked at tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:640:13:
    forge/src/engine/assay.rs names `Command::new` …                                   EXIT=101  ← row 1 (lane/muse)

    $ the same command, row 1's assertion neutralised for one run (temporary; reverted, `git status` clean)
    panicked at tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:924:5:
    the rule's role pattern changed: the lint must name Scout, Assay and Inspector …   EXIT=101  ← row 4 (longcat)
        → section 5 (the mutation-verb sweep, row 3) PASSED in that run; row 4 is the next thing behind row 1

    $ cargo test -p test-harness --test arch_boundary__011__qa_cannot_own_git_mutations   (section 5, by counting)
    outside tests/: forge/src/engine/git_publish.rs — the only file, naming one verb, once (the guard's Some(1))
    the publish gate window: git_publish.rs:166 (FORGE_ALLOW_PUBLISH) → :173 ("push") = 7 lines, limit 12

    $ cargo fmt --all -- --check                                                              EXIT=0
    $ cargo check --workspace --all-targets                            Finished in 27.51s      EXIT=0

    $ pnpm slice:check --since 98ef611dd --receipt /tmp/deep-slice-receipt.md   (the gate's own T1, this slice)
    sections      harness                crates   harness
    T0 compile    PASS (52s)             FMT rustfmt   PASS (2s)
    T1 sections   FAIL (219s) → stopped at arch_boundary__011…rs:640, which is row 1 (lane/muse)
    RESULT        DO NOT HAND OVER — T1 — a test in a section you touched failed
        → this slice's T1 is blocked by row 1 and not by its own change: the four rows it closed were run
          individually (above) and are green. **Rows 2 and 10 are behind row 1 for any `tests/` slice** —
          the section runs `cargo test -p cli -p test-harness` with no `--no-fail-fast`, and
          `arch_boundary__011` sorts before `forge_arch_seam__001` and `runtime_deploy__004`, so neither of
          those can be *observed* in a section run until row 1 lands. Run them by name to see their verdict.

Third pass the same evening (captain: **"FIX"**) — rows 4 and 10 closed, row 7's code half with row 10:

    $ cargo test -p test-harness --test arch_boundary__011__qa_cannot_own_git_mutations
    panicked at tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:644:13:
    forge/src/engine/assay.rs names `Command::new`. … If a QA module genuinely needs this name,
    that is an architecture decision, not a test edit                              EXIT=101  ← row 1 ONLY

    $ the same command, row 1's assertion neutralised for one run (temporary; reverted, `git status` clean)
    test arch_boundary_011__qa_cannot_own_git_mutations ... ok
    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out    EXIT=0  ← section 6 (row 4) passes

    $ grep -F -c 'r"(?i)\b(scout|assay|inspector)\b"' cli/src/forge/lint.rs                    1
    $ grep -F -c 'pattern!(roles_that_may_not_commit, r"(?i)\b(scout|assay|inspector)\b");' cli/src/forge/lint.rs   0

    $ set -a; . ./.env.local; set +a; cargo test -p test-harness --test runtime_deploy__004__server_executable
    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.68s   EXIT=0

    $ env -i PATH=… HOME=… TERM=dumb cargo test -p test-harness --test runtime_deploy__004__server_executable
    test result: ok. 1 passed; 0 failed; 0 ignored; 0 measured; 0 filtered out; finished in 0.29s   EXIT=0

    $ the SAME command on the pre-fix file (`git show HEAD~2:…` for one run, reverted the same session)
    test runtime_deploy_004__server_executable ... FAILED
    panicked at tests/tests/runtime_deploy__004__server_executable.rs:115:13:
    server executable: server should have failed without APP_ENV but is still running
    test result: FAILED. 0 passed; 1 failed; 0 ignored; 0 measured; 0 filtered out
        → the old case asserted a refusal its own child was never given: it inherited the sourced `.env.local`, so
          APP_ENV arrived from the parent and the server served. So the two green runs are a change and not a
          coincidence, and the target now finishes faster than the three fixed 500 ms sleeps it used to pay for
          alone (0.68s and 0.29s, where the failure took 0.51s to reach only its first case).

    $ git status --porcelain            (after both runs)                                    (empty)
    $ ls -d build                       ls: build: No such file or directory      ← row 7's code half, no in-tree build
    $ cargo fmt -p test-harness -- --check                                                   clean

Also closed 2026-10-08 in the same pass: **the rustfmt drift on trunk is gone** — 660 hunks across 182
files, 181 of them under `tests/tests/`, formatted by `6fe65a1a3` with no token change outside rustfmt's own
normalizations (whitespace and commas removed, 177 of 182 files are byte-identical to their previous
revision; 5 more once rustfmt's inserted braces are removed; the 40 with moved imports hold the same
statements reordered). The four trees `gates.yml` defers were untouched. This matters twice over: the
rust-format step sits **above** the test step in `gates.yml`, so while it was red `cargo nextest` never ran —
the job reported "rustfmt would change something" and proved nothing else, which is how rows 8 and 9 stayed
invisible.

**The trunk estate, measured once, 2026-10-08** (`cargo test -p forge -p test-harness --no-fail-fast`, this lane):

    first pass:    594 targets reported — 565 ok, 29 red; 1027 tests passed, 30 failed, 205 ignored
    re-run of the 29 with `.env.local` sourced:
                   23 ok, 6 red — rows 1, 2, 6, 8, 9 and 10, and nothing else

The 23 that turned green are environment-class, not reds: without `.env.local` the DEV-backed families stop at
`DATABASE_URL_DEV is not configured` / `database target is undeclared` (`project_wbs_*`, `sec_entitlement__00[1-7,9]`,
`sec_audit__005`, `service_registry__003-006`). Two of the six are in-memory and would be red in CI as well
(rows 8, 9). That is the whole red set on trunk as of `6fe65a1a3` — and it is the first complete pass anybody
has run since the rust-format step went red.

## Superseded (2026-09-15, Next.js era)

The paragraph that stood here — "Nothing is blocking; `pnpm forge:harness`, `pnpm db:parity`, `pnpm test:app`,
`pnpm smoke:prod` and `tsc` are all green" — described the TypeScript estate, which is retired (`legacy/`,
`docs/agent/LEGACY-TYPESCRIPT.md`). It was kept green by a different toolchain than the one that ships now,
and it never covered the Rust guard targets above.

## Released (2026-09-15)

- **Production is current as of `ce553e9`.** `bash scripts/vercel-build-prod.sh` → `bash scripts/vercel-deploy-prod.sh`
  shipped the harness gates, the board repair, and Factory Phases 1–4; the deploy verified the live sha and
  ended in the live smoke (4/4, including the sha assertion), and protected routes answer 307 to `/login`
  rather than 500, which is what proves the edge middleware and the edge-safe instrumentation load.
- **Two build paths now have to keep working, and that is a new obligation, not a detail:**

  1. `vercel build --prod` → `pnpm run build` → `next build` (**Turbopack**, Next 16's default). It REFUSES a
     project that has a `webpack` config and no `turbopack` config — that error is what broke the first
     release attempt, and it only appeared once the edge fix added a `webpack` key. `turbopack: {}` in
     `next.config.mjs` is the fix, and it must stay.
  2. `pnpm exec next build --webpack` (the QA command in `AGENTS.md`) — this is the path that needs the
     edge-only builtin fallbacks, because webpack walks instrumentation's dynamic import graph.

  Exit: pick ONE bundler. Either make `pnpm run build` be `next build --webpack` so the release runs exactly
  what QA runs, or drop the webpack config and verify Turbopack passes the same edge graph. Until then, a
  green `next build --webpack` does NOT prove the release builds — measured today, webpack was green while
  `vercel build` failed.
- **`node:`-scheme imports are not stubbable in the edge compilation.** `lib/execution-target.ts` and
  `legacy/db/database-gateway.ts` now import bare `fs`/`crypto` for that reason. Exit: if Next stops bundling
  instrumentation for Edge, revert to the `node:` form and delete this line.

## Paid on 2026-09-15 (kept so the register visibly moves)

- **The dual build path is gone: the release runs exactly what QA runs.** `next build --webpack` is now
  `pnpm run build`, so `scripts/vercel-build-prod.sh` → `vercel build --prod` builds with the same bundler
  and the same `next.config.mjs` edge fallbacks the QA command exercises. `turbopack: {}` stays declared
  because the dev server is Turbopack; the webpack edge fallbacks are what make bare `fs`/`crypto` stubbable
  in the Edge compilation. Proven by running the release build after the change (harness gate green inside
  it, artifact produced, nothing deployed).
- **The five `ENG-FORGE-V9` topology failures were not five stale tests — one of them was a live throw.**
  `agent-runtime/forge-topology.ts` retyped the expected FORGE_SDLC version as a literal `1` while the loader
  (`legacy/workflow_app/definitions/forge-sdlc.ts`) had long since exported `FORGE_SDLC_VERSION = 6` and read
  `FORGE_SDLC-v6.xml`. The guard therefore failed closed on a path that `scripts/forge-orchestrate-wake.ts`
  calls (`runForgeHydrate`, `runForgeFollow`), and that script is imported by `scripts/agent-work.ts` — the
  live worker. The throw was gated behind the night plan, so it would have killed **the first unattended
  night run**, which is the same run the learn loop and the ROI backfill are waiting on. Fixed at the source
  of truth: the guard and the test both import `FORGE_SDLC_VERSION` (one constant, no second literal), the
  stale `-v1.xml` header and error messages now name what is actually loaded, and `pnpm test:agent-runtime`
  is 237 pass / 0 fail.
- **The release artifact is never built from a duplicated `.next` tree.** `scripts/vercel-build-prod.sh` now
  clears `.next` before `vercel build` (measured: 2,035 stray files that morning, 0 immediately after a
  clean release build). The upstream duplication is **not** solved — it is an open item below — but it can no
  longer reach a release artifact.
- **The harness now gates the release.** `pnpm forge:harness` runs inside `scripts/vercel-build-prod.sh`
  before the artifact is built, so a drifted manifest, a hand-edited vendor block or a packet citing a dead
  path stops a release. It proved itself on its first run: writing the missing test made the FORGE-GATES-01
  manifest stale and the build aborted rather than shipping the stale artifact.
- **The lint baseline is EMPTY.** Both recorded debts were paid instead of carried: the four V4 packets'
  free-text `## Skills` sections now name `workflow` (the pack that applies), and the three stale citations
  were re-pointed at the files that hold the code today. `pnpm forge:packet-lint`: 0 failure(s), 1 warning(s),
  **0 baselined** — down from 22 warnings with 8 baselined.
- **`agent-runtime/write-policy.test.ts` exists** (7 tests) — the file `docs/agent/packets/FORGE-GATES-01.md`
  listed in its Assay commands, covering the commit boundary and the rewind that makes a non-builder commit
  unreachable from the worktree.
- **Per-card KIND chips** (`lib/sorter-board.ts`, `app/portal/tech/page.tsx`,
  `components/portal/tech/story-kanban-board.tsx`, `legacy/workflow_app/tests/sorter-board.test.ts`): a staged card
  now shows its own kind, and an unread kind renders no chip rather than a default. The page reads
  `listStagingBatchItems()` once and uses it for both the batch roster and the cards.
- **Seven skill packs are anchored** to real paths (forms → `lib/forms/form-instance-io.ts`, neon →
  `legacy/db/database-gateway.ts`, ui → `app/globals.css`, workflow → `legacy/workflow_app/forge/agent-runtime-role-runner.ts`,
  knip → `./knip.json`, cruiser → `./.dependency-cruiser.js`, semgrep → `scripts/forge-packet-lint.ts`), and
  cruiser / knip / ripwire / rtk / semgrep / serena are in `KNOWN_SKILLS`, so packets can actually load them.
- **Stray file deleted:** `app/api/build-info/route 2.ts` (an untracked editor-save duplicate, byte-identical
  to `route.ts`, verified before removing it).
- **Production is current** (`ce553e9`), so the "lags `main` by design" line is gone; it was a fact to
  report, not debt to carry.

## Open, in the order I would pay them

1. **One skill pack has no anchor and says so.** `docs/agent/skills/serena.md` is the single remaining
   `skill-not-anchored` warning (`pnpm forge:packet-lint`: 0 failure(s), 1 warning(s), 0 baselined). Serena
   is a global tool with no config, index or adapter committed here, so there is nothing truthful to point
   it at. Exit: when the runtime adapter exposes serena as a tool, point the pack at the adapter; otherwise
   delete the pack. The pack states this in its own body rather than carrying a fake citation.
2. **Inspector's stale flag still files no work.** `decisionWritePolicy('inspector', 'flag-stale')` is
   written and tested, and Phase 3 has the machinery that could open the item, but nothing connects the two:
   `pnpm forge:decision` has no `flag` command. Exit: add one that opens a `kind=learn` item carrying the
   decision key as its pattern key, then delete this line.
3. **A burst of findings files one item and defers the rest, by design.** The learn loop's window advances
   on every successful pass, so the other nine findings in a noisy commit are reported as `deferred` and are
   not filed by a later pass (the window has moved past them). That is the packet's cap working, but it means
   a big commit's second finding needs a human to notice the `deferred` line. Exit: if this bites, keep an
   unfiled-findings queue (a table, not a log line) and let the loop drain one per pass.
4. **The learn loop has never run unattended on PROD.** It is wired into `scripts/agent-work-entry.ts` after
   the batch fire and is proven on DEV (dry run + the de-dupe probe), but no launchd pass has executed it
   against the live board yet. The first night run is the end-to-end proof; watch `app_error` for
   `forge-learn-pass-failed` and the worker log for the `learn:` lines.
5. **Kind + policy has never been printed by a live claim**, for the same reason: no work item has been
   dispatched since Phase 1. The copy and the log line are unit-tested and the columns exist in both
   environments; the first claim is what makes it observed rather than asserted.
6. **Every ROI row reads `unrecorded` until a post-Phase-1 attempt finishes.** `pnpm forge:roi` on PROD shows
   506 attempts in one bucket because kind/policy were added on 2026-09-15 and no attempt has run since.
   Exit: it resolves itself on the first night run; if it still reads `unrecorded` after one, the copy at
   dispatch is not happening and that is a Phase 1 bug, not a rollup bug. Phase 4's own acceptance
   (a cheap fix and a judgment feature as two rows) is not demonstrated for the same reason.
7. **Something upstream re-creates duplicate files inside `.next`, and they break `tsc`.** The honest
   current state, with the measurements that produced it: **2,035** stray files (`cache-life.d 3.ts` and
   friends) the morning of 2026-09-15; **0** immediately after a release build that started from an empty
   `.next`; **377 back after the next build**, carrying **preserved mtimes** (04:10 and 05:31 on files a
   fresh 11:0x build had just created), which points at a cache restore rather than an in-place copy. `tsc`
   then fails with `TS6200`/`TS2300` duplicate identifiers from `.next/types/*.d N.ts`, and `rm -rf .next`
   clears it (`tsc` clean, verified). Ruled out: no test or script in this repo runs a build, and nothing
   outside `.next` holds a copy. Exit: reproduce with `vercel build --force` (or by clearing the local build
   cache) and name the cache directory that feeds it; until then the release path is clean by construction
   and the local remedy is `rm -rf .next`.

## Deliberately not debt (do not "fix" these)

- **No third model policy, no seventh kind.** `lib/forge-kind.ts` is closed on purpose; the packet's
  stop condition says a third policy is a HOLD, not a feature.
- **`MEMORY.md` still holds the long-form incident narrative.** Phase 2 introduces `forge_decision` as
  the store that outlives an agent; until then MEMORY.md is not a duplicate, it is the only copy.
- **The engine runs PROD only.** DEV is where the Phase 1 probe runs because a probe is not a lane.
- **`cost_usd` is still empty almost everywhere** (vendor-reported actuals arrive late): the ROI strip
  reports widget coverage instead of pretending to know dollars. True before Phase 4 and recorded so nobody
  "fixes" it by inventing a rate.
- **The ROI window is attempts-by-work-item, not spend-by-run.** One story with three attempts counts three
  times, which is the right answer to "what did the night batch cost" and the wrong one to "what did this
  story cost". Exit: a per-story view, if anyone ever asks for one — until then it is a scope choice, not a
  defect.
- **Two modules import bare `fs`/`crypto` instead of the `node:` scheme** (`lib/execution-target.ts`,
  `legacy/db/database-gateway.ts`). The `node:` form is not stubbable in the Edge compilation, so the bare form is
  load-bearing. Exit: if Next stops bundling instrumentation for Edge, revert to `node:` and delete this.
