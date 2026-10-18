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
being the same file. So **two red targets remained, and both looked like a decision rather than a repair**:
`arch_boundary__011` (row 1 — the assertion whose own message says a QA module needing that name "is an architecture
decision, not a test edit"; rows 3 and 4 sat behind it and are closed, so it stands alone in that guard) and
`forge_arch_seam__001` (row 2, the job layer reading `opencode::turn_ceiling`). Each wanted one word from the captain
— **WIDEN** the detector or **MOVE** the call — and no lane may decide it for him.

Updated **2026-10-13** (`Culebraluxe-web`, main checkout). **Row 2 closed — by the tree, not by a ruling**:
`turn_ceiling` now lives in `engine::config` (`config.rs:12`, re-exported by `opencode/config.rs:3`), so
`engine/job` names no vendor at all and `the_job_layer_and_registry_know_no_role_no_node_and_no_vendor` is green over
the layer's two files. The same measurement moved a **new** red onto row 1 and closed nine others behind it:
**WO-406 (`f685cdae1`) moved five module files into directories** — `engine/job.rs` split into
`job/{mod,workflow_service,tests}.rs`, and `engine/opencode.rs`, `roles/lifecycle.rs`, `roles/qa.rs`, `roles/smith.rs`
moved to `…/mod.rs` inside the same-named directories that already held their submodules (R100, code byte-identical)
— and **left every rail that READS those paths naming a file that no longer exists**. Nine test targets were red on
the trunk for it, all one signature: `src/source.rs:71` panicking "must be readable", or a pin refusing a file that
is gone. Two were hidden behind `job.rs` in the same list (`roles/lifecycle.rs`), which is why the count grew from
seven to nine as each fix let the next failure surface. All nine are repointed and green in **`7bce56291`**:
`forge_arch_seam__001/002/003/004/006`, `forge_job__013`, `arch_boundary__012` (its `LANE_MODULES` pin — the
deliberate edit its own message asks for), `forge_assay__005` (its `QA_LANE`), plus the `arch_one_writer__004`
planted sample. `arch_boundary__012`'s sweep also read the extracted `engine/job/tests.rs` as **production** code —
a test module carries no `#[cfg(test)]` of its own, its parent declares it — so one was added to it and the deploy
pin caught the phantom reader; the sweep now skips the crate's test-module names (`tests.rs`, `*_tests.rs`).
**Row 1 changed shape with it**: it no longer fails on `assay.rs` naming `Command::new` but at `:628`, the QA-surface
pin, because the surface scan matches FILE NAMES (`is_qa_surface_name` — stem starts with `qa`/`assay`) and
`roles/qa/mod.rs` is named `mod.rs`, so **the QA lane left the surface that guard scans**. The pin's own message says
what to do ("If one was RENAMED away, say so here too"), but both readings are a guard-strength choice —
**WIDEN** the name rule to see a module root inside a `qa*/` directory, or **PIN** the rename and accept that lane
root is no longer scanned — and row 1 is the captain's either way. So **one red target remains that no one has ruled
on**: row 11, `forge_assay__011`, behavioural and unrelated to WO-406.

The previous review was 2026-09-15
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
| 1 | **`arch_boundary__011` refuses `forge/src/engine/assay.rs` naming `Command::new`.** The QA surface owns no process spawn; FIX-005 added one to bound every wait (`spawn_scoped_shell`, `spawn_scoped_shell_with_env`: `assay.rs:203`, `:232`; `:764` in its `#[cfg(test)]` module). | Assert `tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:632`; name arrived in `bd1b093f5` (FORGE-FIX-005, merged `530903627`) | **lane/muse** — author of FIX-005 | 2026-10-10 | **CLOSED 2026-10-13 (`6985ceb10`)** — `cargo test -p test-harness --test arch_boundary__011__qa_cannot_own_git_mutations` → **1 passed**. Three rename-shaped breaks sat in front of the `Command::new` assertion this row was opened for, all of them WO-406's, and all now fixed at the guard: (a) the QA-surface pin names the rename — `QA_SURFACE` carries `forge/src/roles/qa/mod.rs` and `is_qa_surface_path` reads a module root by the DIRECTORY it lives in, so the QA lane is scanned again instead of falling out of the surface (`:628`); (b) the node-table sweep skips the crate's test modules by name (`is_test_module`: `tests.rs` / `*_tests.rs`) — WO-406 extracted `engine/job/tests.rs`, and it was read as a production file naming every lane (`:886`); (c) the export pin reads the new path (`:939`). The `Command::new` question closes as **overtaken, not granted**: the guard already exempts `forge/src/engine/assay.rs` by name at `:656` (`path != "forge/src/engine/assay.rs"` — `assay.rs` *is* the bounded assay runner its own message points at), so the spawn FIX-005 added stays and every other QA file is still refused one. No spawn was legalised here |
| 2 | **`forge_arch_seam__001` refuses `forge/src/engine/job.rs` naming `opencode`** — the job layer reaches the vendor to read a turn ceiling (`job.rs:542-544`, `crate::engine::opencode::turn_ceiling`). | Assert `tests/tests/forge_arch_seam__001__canonical_execution_chain.rs:281`; name arrived in `32832c57c` (FORGE-FIX-006, merged `1a0f60269`) | **lane/nemotron-2** — author of FIX-006 | 2026-10-10 | `cargo test -p test-harness --test forge_arch_seam__001__canonical_execution_chain` green. **CLOSED 2026-10-13 (`7bce56291`)** — by the tree, not by a ruling: `turn_ceiling` moved to `engine::config` (`config.rs:12`, re-exported by `opencode/config.rs:3`, in `f685cdae1`), the rail reads the layer's two files (`job/mod.rs` + `job/workflow_service.rs`), and `the_job_layer_and_registry_know_no_role_no_node_and_no_vendor` is green |
| 3 | **Behind #1 — the mutation-verb sweep counts a test file.** The sweep requires that only `forge/src/engine/git_publish.rs` names `"push"`/`"merge"`/`"rebase"`; measured, exactly three files in the workspace name such a literal — `git_publish.rs` (allowed), this guard (excluded by `SELF`), and `tests/tests/forge_assay__005__qa_cannot_modify_git.rs:32-34`, a test whose subject *is* that QA may not push. | Assert `arch_boundary__011…rs:880`, `workspace_code()` (`:395-405`) sweeps `tests/**` | **lane/deep** | 2026-10-10 | **CLOSED 2026-10-08 (`1c55a567b`)** — `TEST_TREE = "tests/"` is that one named, dated line: the sweep reads the production tree only, where exactly one file names a verb (`forge/src/engine/git_publish.rs`, once — the guard's own `Some(1)`), and section 5 is green *observed*, by a run that reached section 6 (`:924`). Receipt at the foot |
| 4 | **Behind #1 — the lint's `pattern!` spelling.** `ROLES_THAT_MAY_NOT_COMMIT` (`arch_boundary__011…rs:220-221`) pins the *one-line* spelling of the lint's role pattern; `cli/src/forge/lint.rs:105-108` now spells it wrapped across three lines, so `lint.contains(…)` cannot match. | Assert `arch_boundary__011…rs:924`; `cli/src/forge/lint.rs:105-108`, wrapped by the rustfmt pass `430a13761` | **lane/deep** — landed the fix `lane/longcat` had written but not pushed (`3b32cc4ff`, batch 28) | 2026-10-10 | **CLOSED 2026-10-08 (`0be8e4949`)** — the assertion names the regex the lint must name (`r"(?i)\b(scout|assay|inspector)\b"`, `arch_boundary__011…rs:233`, asserted `:927`), not the macro's line breaks, and it was **observed**: with row 1's assertion neutralised for one run (temporary, reverted the same session, `git status` clean) the guard is **green** — `1 passed; 0 failed`, `EXIT=0` — so sections 5 and 6 both pass on trunk and row 1 stands alone in that guard. Receipt at the foot |
| 5 | **Six SEC.REDIRECT story ids carry two test files each.** Three batches authored tests for the same stories and nothing says which file answers a story id: `b3ce7fef9` (2026-10-07, batch 45 — its own subject: "blocked by web crate compilation error") added `001__path_traversal`, `002__open_redirect`, `003__host_injection`, `004__percent_encoding`, `005__fragment_handling`, `006__query_parameters`; `68ba004de` (2026-10-08) added the ids' canonical files as `docs/agent/proposals/TST-REDIRECT-2026-10-08.patch` names them (`001__relative_path_allowed`, `003__evil`, `004__evil`, `006__userinfo_tricks`, and the single `007`/`008`/`010` files); `f438d7a68` (2026-10-08) added `002__same_origin_absolute_behavior_as_intended`, `005__encoded_slash_backslash_variants`, `009__malformed_unicode`, `011__redirect_loops`. | `ls tests/tests/sec_redirect__*.rs` — 17 files, ids 001–006 doubled | **lane/muse** — author of all three batches (every one of the three commits arrived on `lane/muse`) | 2026-10-10 | One file per id: each doubled pair folds into the canonical file — batch 45's assertions move inside it — and batch 45's name is deleted, so the id-to-file map is a function again. Naming the canonical half is done (that is `docs/agent/HANDOFF-TST-REDIRECT-2026-10-08.md`); the fold is not |
| 6 | **`prop_property_based__008__monetary_arithmetic` is red on trunk, and the stale half is the test.** It asserts `format_money("1250000") == "$1,250,000"` and `"1250000.5" == "$1,250,000.5"`; `middle/model/src/forms_format.rs:12` was rewritten for forms v5 by `c483e8f81` to always write two decimals (`"$1,250,000.00"`, `"$1,250,000.50"`), and the **passing** test `tests/tests/docs_forms_template__007__field_formatting.rs:42-43` pins that two-decimal contract. Found by the trunk estate run 2026-10-08: `test prop_property_based_008__monetary_arithmetic ... FAILED`, `left: "$1,250,000.00"`, `right: "$1,250,000"`. The formatter is the canonical half — its own doc says two decimals and a second test agrees. | Assert `tests/tests/prop_property_based__008__monetary_arithmetic.rs:76-77`; batch 36 (`17bf934a6`, authored against a tree without `c483e8f81`) | **lane/muse-2** — batch 36's lane | 2026-10-10 | **CLOSED 2026-10-08 (`1c55a567b`)** — those two assertions take the v5 contract, **and so does the display property behind them**, which was failing on `amount = "0000"` (0 commas, 1 expected) and had been masked by the two ahead of it: grouping is now measured on the *rendered* whole part and the output must carry exactly two decimals, which is stronger and true (`"0000"` → `"$0.00"`, `999.999` → `$1,000.00`). `cargo test -p test-harness --test prop_property_based__008__monetary_arithmetic` → ok, 1 passed. The hostile-input assertions are untouched, and the new failing seed is committed |
| 7 | **A test builds the whole workspace inside the checkout.** `runtime_deploy__004__server_executable.rs:45` falls back to `<checkout>/build/rust` when `CARGO_TARGET_DIR` is unset — the shared target dir retired 2026-10-07 — so running it does a **release build inside the tree**, at a path no `.gitignore` covers (`/target/` is ignored, `build/` is not). Measured 2026-10-08: the directory appeared mid-run and `git add -A` died on `build/rust/release/deps/rmeta…/full.rmeta`, a file the build was still rewriting. Two guards also describe `build/rust` as the shared dir in their doc comments, so the path reads as current. | `tests/tests/runtime_deploy__004__server_executable.rs:44-52` (batch 41, `ebed4727a`); the only test with that fallback (`grep -rl 'join("build")' tests/tests/` → 1) | **lane/muse** — batch 41's lane | 2026-10-10 | **Code half landed 2026-10-08 (`e83da7256`, lane/deep)** — `cargo_target_dir` (`runtime_deploy__004…rs:99-119`) asks cargo instead of guessing: `CARGO_TARGET_DIR`, then `cargo metadata`'s `target_directory` (which honours each lane's `.cargo/config.toml`), then cargo's own default `<root>/target`; the `CARGO_MANIFEST_DIR` fallback that named `lane-nemotron` is compile-time now. Measured after the target ran twice: `ls -d build` → **No such file or directory**, `git status --porcelain` → empty. **Still open: the doc-comment half** — five guards (`arch_boundary__012…:115`, `arch_one_writer__002…:64,210`, `_003…:54,157`, `_004…:49,145`, `arch_route_map__006…:59`) still describe `build/rust` as shared by every lane. That prose is lane/muse's row; lane/deep did not write in another lane's guards |
| 8 | **`wf_token__006__optional_branch_cannot_prevent_completion` — the process stays `Active` after every required token completes.** In-memory (`EngineHarness`), so it is red in CI too and not a database artefact; the run that found it (2026-10-08) is the first that ever got this far, because the rust-format step above the test step in `gates.yml` was red until `6fe65a1a3`. Which half is stale is a **workflow-engine call**, not a test edit: the test pins "completion is gated on required tokens only". | Assert `tests/tests/wf_token__006__optional_branch_cannot_prevent_completion.rs:271`; batch 60 (`5fdfbf4ae`, 2026-10-08) | **lane/deep** — moved from lane/muse by the captain 2026-10-08 ("assign 8 9"); the `middle/workflow` half needed no change | 2026-10-10 | **CLOSED 2026-10-08 (`1c55a567b`)** — the ruling is that the **engine is right and the case was stale** (see the workflow ruling below). Completion is `count_active_tokens == 0` (`execute_node_leave.rs:173-202`); an optional branch is retired at the **join** (`handle_join.rs:42-61`), which is the shape `RE_supermodel-v1.xml:199-204` relies on. The graph now has that join, its retired sibling lands `Completed`/`Skipped` with its task obsoleted, the process completes, and the retired task refuses completion. Target green |
| 9 | **`wf_token__007__required_branch_does_prevent_completion` — a cancelled process's tokens carry `TokenOutcome::Completed`.** The instance does reach `Aborted`; the tokens do not carry `Cancelled` (`left: Some(Completed)`). Also in-memory and CI-visible. Same rule question as row 8, opposite direction: whether a mass cancel must write its own outcome or may reuse the completion path is the engine's decision, and the test asserts an implementation detail it also documents as one ("`complete_token` is called with `Cancelled` outcome"). | Assert `tests/tests/wf_token__007__required_branch_does_prevent_completion.rs:332`; batch 60 (`5fdfbf4ae`) | **lane/deep** — moved from lane/muse by the captain 2026-10-08 ("assign 8 9"); the `middle/workflow` half needed no change | 2026-10-10 | **CLOSED 2026-10-08 (`1c55a567b`)** — cancellation already writes its own outcome: `terminate_process` marks every token it catches active `Cancelled` (`execute_node_leave.rs:223-238`). The assertion was over-broad — it read **every** token of the instance, including the fork's own parent, which `handle_fork` had concluded `Completed` before the cancel (`:385`) and which cancel must not rewrite. It now asserts the caught tokens carry `Cancelled`, that every token is concluded either way, that the one pre-concluded token is the fork's parent keeping `Completed`, and that no task is left open. Target green |
| 10 | **`runtime_deploy__004__server_executable` fails on its own environment handling, not on the server.** Its negative case spawns the built `web` with `PORT` set and `APP_ENV` deliberately omitted — but it does not *clear* the child's environment, so it inherits whatever the test process has: run with `.env.local` sourced (every lane) the child starts and serves, and the test fails. Measured 2026-10-08 with the environment actually cleared (`env -i PATH=… PORT=8099 ./build/rust/release/web`) the binary exits with `DbFailure … "database target is undeclared; set APP_ENV or use VERCEL_ENV"` — the refusal is real, and the child is not given the conditions the case needs. The 500 ms wait before `try_wait` adds a second flake. | `tests/tests/runtime_deploy__004__server_executable.rs` — the four negative cases (batch 41, `ebed4727a`); the child env is set at `:71-80`, the waits at `:43-62` | **lane/deep** — batch 41's lane left the case as it found it; the fix landed here | 2026-10-10 | **CLOSED 2026-10-08 (`e83da7256`)** — `refusable_command` (`:71-80`) clears the child's environment and keeps only `PATH`, so "APP_ENV intentionally not set" is true of the process the server sees; `wait_for_exit` (`:43-62`) polls on a bound (30 s to refuse, 5 s for SIGTERM) instead of sleeping 500 ms. Green both ways: `.env.local` sourced → `1 passed; 0 failed … finished in 0.68s`, `EXIT=0` (the run that used to fail) and under `env -i PATH=… HOME=…` → `1 passed; 0 failed … finished in 0.29s`, `EXIT=0`. Receipt at the foot |
| 11 | **`forge_assay__011__artifact_written_before_lane_loses_result` — a lane whose result is lost AFTER the run pointer is stamped keeps `0` artifacts (`left: 0, right: 1`).** Behavioural, not structural: the rail reads the production capture path and fails on ordering, and its own message is "the run pointer is stamped before the capture it points at — which is exactly why the capture failure must fail the lane instead of passing silently". WO-406 (`f685cdae1`) touched no assay file, so this is not the refactor's collateral; measured red on the trunk 2026-10-13, and it is the one red target no one has looked at. | Assert `tests/tests/forge_assay__011__artifact_written_before_lane_loses_result.rs:214`; the rail arrived in `02438eb63` (C1–C4) | **unassigned — needs the captain's owner and expiry** | — | `cargo test -p test-harness --test forge_assay__011__artifact_written_before_lane_loses_result` green |


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

0. **FORGE-B1 is one slice in of four.** Slice 1 (the claim fence, migration 278) landed on 2026-10-09 as
   `90b1a9619` and is verified on DEV: a superseded execution can no longer begin a run, refresh its replacement's
   lease, or settle its replacement's work, and settlement idempotency is derived by the routine from
   `(item, generation, outcome)` instead of a caller-supplied key. **Slices 2, 3 and 4 are open** — defect #2
   (completion effects are four separate writes, not one transaction), defect #1 (`reconcile_completions` skips on a
   global watermark, reads one active instance, and caps at 200 events) and defect #7 (a stale snapshot can demote a
   completed story, and the hold routine moves the board even when its item update matched nothing). The plan, the
   entry points and the receipts are in `docs/agent/HANDOFF-FORGE-B1-2026-10-09.md`. **Migration 278 is on DEV and PROD**
   (applied to PROD on the captain’s word 2026-10-09, same checksum, three signatures verified), so the
   operational hold that sat on top of these slices is closed.
   Exit: the hand-off's §6, in order.

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

## Agent OS Phase 2 (GPT contract) — landed vs deliberately deferred (2026-10-18)

The Phase 2 contract was ambitious; this records the part that fits the current stack and the part
that stays parked rather than lost.

**Landed:**
- `forge/src/engine/executive_swarm.rs` — the typed contract (4 executives + 8 experts, deterministic
  routing, Council gate, limits, `ExpertVerdict`, `DecisionSynthesis`, escalation, `ForgeDecisionHandoff`).
- `forge/src/engine/advisory_profiles.rs` — one backend-neutral policy table for the twelve ids, consuming
  the contract's catalogue; read-only, bounded steps, canonical skills, web only where justified.
- `forge/src/engine/dsh/advisory.rs` + `forge/src/engine/advisory_runner.rs` — a Phase 2 panel executed
  through the DSH harness: independent read-only seats, verdict/synthesis JSON validation, no writes.

**Landed 2026-10-18 (P1/P2/P3 of the Phase 2 integration):**
- **P1 durable decisions/HOLD/handoff.** `db::NewForgeDecision` + `ForgeEngineDao::insert_decision`
  write a `candidate` row (idempotent by key, never self-promoted); `decisions::record_panel_decision`,
  `record_handoff_artifact`, and `open_panel_hold` persist the panel; `PanelOutcome::record` writes the
  decision, attaches the `ForgeDecisionHandoff` as a story artifact when a story id is supplied, and
  opens a HOLD when a story + workflow instance are supplied. Draft-story auto-creation is NOT done —
  see remaining item 5.
- **P2 Council rounds.** `advisory_runner` now runs the three rounds already specified in
  `docs/agent/executive/COUNCIL.md`: independent Round 1, cross-examination Round 2 (Council only),
  and Round 3 synthesis; `SeatRunner` makes the protocol testable without a model call. Per-seat tool
  differences are materialized where enforcement exists: OpenCode advisory profiles carry the web
  allowlist; the DSH panel still uses one read-only sandbox (remaining item 1).
- **P3 materializers.** `opencode_agents::ADVISORY_AGENT_PROFILES` renders the twelve profiles from
  `advisory_profiles.rs` (single catalogue, read-only, ADVISORY_MAX_STEPS, web only for the four
  research seats) with rails (`v2_advisory_profiles_render_read_only_from_one_catalogue`); the
  regenerated `opencode.json` carries them. The Maestro adapter now passes `--read-only` for Forge's
  read-only role nodes (vendor-side enforcement).

**Remaining TD:**
1. **DSH per-seat agent presets.** Headless is a rosterless deployment; per-seat tools on DSH need a
   composed agent-presets profile (start with two read-only variants: with/without web).
2. **Read-only web research on DSH.** The OpenCode web allowlist exists; the DSH panel needs a
   read-only web preset plus an audit of what it can read.
3. **Advisory spend rates.** DSH records tokens and the adapter parses them; cost needs
   `DSH_PRICE_INPUT_PER_MTOK` / `DSH_PRICE_OUTPUT_PER_MTOK` / `DSH_PRICE_CACHE_READ_PER_MTOK`.
4. **Micro-agent children in advisory seats.** Contract allows 3/depth 1; DSH subagent spend join is
   unproven, so the stricter bound (none) remains.
5. **Handoff auto-create vs attach.** Current behavior: persist the handoff as a story artifact when a
   story id is given; creating a Story Board draft automatically is a product decision (not debt by
   accident).
6. **Maestro advisory roster.** The OpenCode profile half is rendered; a Maestro roster agent per
   advisory role and the external roster configuration remain an operational step.
7. **OpenCode/Maestro seat runners.** The panel executes through DSH today; add `SeatRunner`
   implementations for the other transports when the panel should run there.

**Not debt, by rule:** no fourth agent registry, no executive nodes in `FORGE_SDLC-v6.xml`, and no
swarm write authority. The contract module and the advisory catalogue are the single source.

## Agent OS Phase 3 (self-learning / earned autonomy) — reviewed 2026-10-18

**Verdict: do not build a "learning engine" or a second telemetry store.** Most of Phase 3 is
deferred; two read-only slices are worth doing now because the durable rows already exist.

**Already exists (do not rebuild):**
- `forge/src/engine/learn.rs` — Rust learn pass: hunts silent-failure patterns + stale claims in a
  window, files AT MOST one learn story per pass, dedupes by pattern key, never fixes/merges/promotes.
- `forge doctor` (control-plane health), `forge roi` (widgets, explicitly not dollars; null ≠ zero),
  `forge/src/engine/profile_analysis.rs` + `profile_report.rs` (cost per accepted candidate by
  profile/turn, unpriced turns never imputed as zero).
- Durable outcome rows: `forge_engine_task_execution`, `forge_role_contract`, `forge_role_finding`,
  `forge_tool_artifact`, `forge_hold_record`, `forge_decision`, run usage/trajectory, QA receipts.
- Enforcement rails: `action_profiles`, OpenCode permission rendering, arch-boundary/citation/
  manifest/repo guards. There is no autonomy engine, and none should be added yet.

**Landed 2026-10-18 (both read-only slices):**
1. **`forge scorecard`** (`forge/src/scorecard.rs` rules + `cli/src/forge/scorecard.rs` command): per
   role/agent rows over `storyboard_story_run` + `forge_hold_record`; verdict classification, rework,
   holds, cost/wall coverage. Honesty: null is not 0, unpopulated reads NOT CAPTURED, widgets are
   never dollars, vendor USD only from `cost_source='vendor'`. `cargo test -p forge scorecard` 17/17.
2. **Daily proactive digest** (`scripts/orchestrate-status.sh --digest`): queue/stuck/HOLD/failed/
   decisions one-pager over the read-only `forge sql` path; the 15-minute stuck window reuses the
   engine's `forge_reapable_claims` rule. Existing human/JSON modes unchanged.

Remaining Phase 3 TD: DSH per-seat web presets, skill-invocation capture, model/role linkage,
correction/hypothesis ledgers, routing metrics, earned autonomy (owner-approved only), drift report,
freshness/audits, Council calibration, and self-modification rails.

**Tech debt, with exits (the rest of Phase 3):**
- **Correction → specification → rail (3.3).** High value, but it needs a durable correction→rule→
  guard→test pipeline AND an owner gate; build it only next to the rail that enforces it, never as a
  free-text self-writer.
- **Hypothesis ledger (3.4).** Needs a durable store and review dates; start after scorecard data exists.
- **Improvement backlog (3.5).** The learn loop already files one story/pass; extend its pattern
  sources after the scorecard, do not add a second backlog.
- **Routing effectiveness (3.6).** Needs route/agent/model attributes captured per run; profile_analysis
  covers cost per accepted candidate, not route accuracy.
- **Earned autonomy / automatic demotion (3.7–3.8).** Requires a policy engine, owner approval for
  promotion, and a safety-first demotion design. Do not let learning grant its own authority.
- **Architecture drift scan (3.9).** Boundary/citation/manifest/repo guards already enforce; a periodic
  drift REPORT is later.
- **Freshness / revisit triggers (3.11), weekly/monthly audits (3.12).** After the daily digest.
- **Skill effectiveness (3.13).** No skill-invocation log exists; needs capture first.
- **Model/role fit (3.14).** profile_analysis is partial; needs role × model linkage per turn.
- **Council calibration (3.15).** Needs decisions linked to later outcomes; after scorecards + the
  durable decision rows.
- **Self-modification boundary (3.16).** Mandatory rails before ANY learning write API: observation,
  scorecard and hypothesis writes are allowed; authority/QA/release/workflow changes stay owner-only.

**Not debt, by rule:** no new telemetry database (learn from durable Forge rows), no automatic
authority promotion, and no learning agent that can publish or edit code.

## Harness Evolution (Harness²-inspired) — v1 landed 2026-10-18

Native Rust subsystem `forge/src/engine/harness_evolution/` (no Python sidecar, no new queue, no
second telemetry store). Built by Architect/Lead/Smith/QA agents: Propose → Probe → Contrast →
Compose → Forge Validate → Adopt/Reject → Reflect.

**Landed:**
- immutable harness revisions (`HarnessManifest` hash, `HarnessRevision::from_changes`, closed lifecycle);
- protected-surface policy (`validate_candidate` rejects RED/authority/QA/production/tool-MCP/XML/
  destructive changes BEFORE probing; adoption policy Adopt only for GREEN+clean, YELLOW→Council,
  RED→Chris);
- `HarnessCatalog` (atomic activate pointer, rollback to exact prior revision, one active per domain);
- probe contract pinning full identity (+ mode/k/seed digest) and refusing production;
- `HarnessExperiment` with k=1 default, injected Propose/Contrast/Compose/Validate/Probe stages, and
  failure isolation (a failed experiment never touches the active harness);
- adoption module (only caller of activate; explicit Reject/MoreEvidence/Council/Chris never move
  the pointer);
- behavioral contrast over 16 dimensions with identity-mismatch invalidation;
- evidence-linked composition (may keep the base; successor immutable);
- Forge validation gates (static policy, regression rails, replay, Phase 3 evidence) where a
  contrast alone is insufficient;
- domain-scoped experience library requiring contrast + complete validation, capped retrieval,
  scope = domain+executor+mode+budget+seed;
- additive migration `db/migrations/294_forge_harness_evolution.sql` (9 tables, FKs to existing
  evidence, one-active-per-domain index) — **deliberately NOT applied** (migrations are human
  holds per WO-07);
- `docs/agent/HARNESS-EVOLUTION-ARCHITECTURE.md`; 162 module tests.

**Completed 2026-10-18 (items 1–7):**
1. Migration 294 applied to **DEV and PROD** (`pnpm db:migrate ... dev|prod`; parity 0/0/0/0) and a
   durable `DbRevisionStore`/`ForgeHarnessDao` written: hash-verified revisions, component projection,
   lineage CRUD, and ONE transactional pointer move (lock → expected-from check → status writes →
   activation row → commit; a failed write leaves pointer and statuses untouched).
2. Replay probe infrastructure: `probe_replay.rs` loads a historical story/run from the control plane,
   materializes the candidate harness content, and runs reference+candidate through an injected
   `ReplaySandbox`; production is refused before loading/execution, evidence is linked by durable ids
   only. The concrete sandbox runner (launching a model turn in a sandbox) remains the next wiring.
3. LLM proposer: `proposer_llm.rs` implements `Proposer` over the existing read-only advisory
   `SeatRunner`, strict JSON with byte/shape limits, policy rejection of RED/authority changes before
   returning; fake-runner tests only (no live model calls in tests).
4. Phase 3 feed: `feed.rs` converts read-only scorecard buckets and repeated HOLD groups into typed
   `ImprovementTarget`s (conservative thresholds, evidence refs required, deterministic/capped), which
   `propose.rs` consumes; the scorecard and daily digest also expose the optimizer surface (digest
   `HARNESS` line).
5. k>1 recursion: `run_recursion` supports parallel (shared evidence, k compositions) and sequential
   (chained passes) modes under `EvolutionBudget`, ceilings enforced before any probe; k=1 unchanged.
6. Operator surface: `forge harness-evolution status|revisions|experiments|experience` (read-only,
   `--format json`) + `pnpm forge:harness-evolution`; daily digest shows active revisions per domain.
7. Validator seam D3: `ValidationContext.changes` carries the exact composed changes into
   `ForgeValidationStage`; D4 transactional activation implemented and tested (failed store write
   leaves active pointer/status unchanged).

**Remaining TD (production wiring, not prerequisites):**
- A real `ReplaySandbox` implementation that launches Forge/OpenCode in a sandbox (the trait and
  production refusal exist; `UnavailableSandbox` refuses rather than pretending).
- Wire `ForgeHistoricalTaskLoader` + the sandbox into the dogfood experiment (3–5 historical Rust
  stories, prompt/tool-guidance only, k=1) and record the first experiment rows.
- Feed `learn.rs` correction/hypothesis findings (beyond scorecard buckets and HOLD groups) and write
  experiment outcomes back without a second learning store.
- Automatic adoption stays OFF (effective-GREEN policy only; no production evidence yet).

QA D1 (adoption re-check with an empty change list / kind-floor-only adoption) and D2 (probe-reading
id mismatch) were fixed before landing: adoption re-runs `policy::validate_candidate` with the
canonical `changes_between` diff, and adoption classifies the effective authority of changed
surfaces; module tests are 170/170.


## Tech-debt closure — 2026-10-18 (afternoon)

All remaining named items were built rather than deferred:
- **Concrete sandbox runner** (`sandbox_command.rs`): allowlisted absolute program, args vector (no
  shell), fresh workspace, env-clear, hard timeout, capped captured output, durable execution refs;
  production refused before spawn.
- **First dogfood plan** (`dogfood.rs` + `scripts/harness-dogfood.sh`): Rust backend, 3–5 stories,
  k=1, prompt/tool-guidance only, policy-checked candidate revision and matched ProbeContract per
  story; dry-run prints the plan and never calls a model or moves the pointer.
- **Correction ledger** (migration 295, now applied to DEV and PROD; `db/src/forge_corrections.rs`):
  every correction occurrence, normalized key, repeat grouping, nullable root-cause/spec location,
  experiment FK; no second outcome store.
- **Learn/hypothesis feed + outcome writeback** (`feed.rs`): repeated corrections become
  `ImprovementSignal::RepeatedCorrection`; learn findings become typed review targets (not confirmed
  weaknesses); adopted experiments write experience, rejected ones write corrections, other verdicts
  write nothing.
- **Routing/model-role metrics** (`metrics.rs`): per role/model attempts, outcomes, completion,
  rework, cost coverage; unobserved/unpriced are `None`, never 0/100%.
- **Council calibration** (`calibration.rs`): Brier-style score from stated probability + observed
  outcome, unscored when data is missing, out-of-range probabilities are typed errors.
- **Automatic adoption remains OFF** (effective-GREEN policy only; no production evidence yet).

Remaining honest limits: the sandbox is a command runner (exit status stands in for a model turn,
cost `None`, receipt is a local file not a probe DB row); the dogfood experiment is a dry-run plan
until a real probe entrypoint records probe rows; `forge harness-evolution probe` is not a CLI
command yet.
