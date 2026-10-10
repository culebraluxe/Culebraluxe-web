# HANDOFF — the Forge live cockpit, landed on main (2026-10-02)

Written by the next agent, because the session that built this stopped without writing one. It crashed mid-work and
left 19 commits on `forge-service-finish-20261002` — a branch the House Rules forbid — while `origin/main` sat at
`b01f4692`. By rule 1 the work did not exist to anyone else, and by rule 8 it was not done: the branch's `rust core`
died on a compile error before any test ran. Those 19 commits are on `main` now, with five cleanup commits on top,
and this file travels with the same push.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | The 19 commits (`513f4afd`..`5f5fc052`) and five cleanup commits — `1a683113` (the Live selection), `e7259f76` (rustfmt), `f58d8db8` (fixture rename), `0a9623fb` (workflow imports), `b2d1da54` (the orphan `result_runs`) — plus this file and the two commits below (`a75859c9`, `8e2794ab`) are on `main`, fast-forwarded from `b01f4692`: no merge commit, no branch left behind | `git log --oneline origin/main \| head -29` |
| S2 | `main` was red **before** this stack: `gates` failed at `b01f4692` (2026-10-02 11:27) on the pending rustfmt backlog and on three fixture keys gitleaks read as secrets. Neither is the cockpit's fault and both are fixed here | `gh run list --branch main --limit 3` |
| S3 | The stack is 58 files and **every one is a modification** — no file is added: 10 in `forge/src/roles`, 5 in `rust/test-harness/tests`, 5 in `middle/model/src`, 4 in `web/src`, 4 in `forge/src/engine`, 4 in `db/src`, 7 under `web/ui/src/app/screens/tech`, 2 in `middle/workflow/src`, and singles | `git diff --name-only b01f4692..origin/main \| wc -l` |
| S4 | The three fixture keys now say what they are instead of being excused: `definition-kinds-under-test-not-a-secret`, `definition-self-loop-under-test-not-a-secret`, `definition-orphan-under-test-not-a-secret` | `rust/test-harness/tests/wf_definition__005__missing_target.rs:64,86,221` |
| S5 | Only `VALID_DEFINITION` (`:51`) is asserted; the three renamed keys are parsed and never compared, so the rename weakens no test | `rust/test-harness/tests/wf_definition__005__missing_target.rs:113,144` |
| S6 | `.gitleaksignore` gained three fingerprints, naming the two commits that carried the old literals, each with its reason written beside it. The file's own doctrine — "rename the fixture, not a line here" — is repeated in that block | `.gitleaksignore` (the 2026-10-02 block) |
| S7 | The Cockpit tabs are named by operating purpose, Work in Flight is wired to Forge live data, and the live metric values are borrowed rather than cloned | `web/ui/src/app/screens/tech/view.rs`, `web/ui/src/app/screens/tech/view/assembly.rs` |
| S8 | PR #35 reads **MERGED** (2026-10-02T17:44:38Z, head `5f5fc052`) because its commits are on `main` — nothing was merged into it and no branch was pushed. The goal "the screens are in `main`" is met; "the checks are green" is not, for the two reasons in §5 | `gh pr view 35 --json state,mergedAt,headRefOid` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | Three uncommitted files in `/Users/lisapenfieldicloud.com/Documents/Culebraluxe-web`: `rust/Cargo.lock` (modified), `db/loads/arm_recovery_batch_2026_10_01.sql` and `db/loads/settle_landed_candidates_2026_10_01.sql` (untracked) | the live lane working in that folder | do not commit, stash, clean or reset them. That lane's next `git pull --rebase` will complain about the modified lock; clearing it is their call, not yours. That lock is the same repair now on `main` (`a75859c9`), so it can be discarded without losing anything |
| H2 | The four deferred trees: `cli/src/apple_mail`, `cli/src/forge/lint` (`lint.rs` **and** `lint/`), `middle/model/src/applemail`, `middle/model/src/apple_messages` | the captain — the fmt gate defers them by name (`.github/workflows/gates.yml:338`) | never run `cargo fmt --all` across them and never hand-format them. `cli/src/forge/lint.rs:103` defines `pattern!(roles_that_may_not_commit, r"(?i)\b(scout\|assay\|inspector)\b")` as **source text**, and `rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:193` pins that exact string: rustfmt reformats it and the assay fails. Hit this while cleaning up; reverted |
| H3 | `origin/forge-service-finish-20261002` and PR #35 | the captain | never push to it — the pre-push hook refuses any branch but `main`. Its commits are on `main` now, so PR #35 should read as merged; deleting the remote branch is a GitHub-UI job |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Change what the Forge live cockpit shows | `docs/agent/COCKPIT-PURPOSE.md` — the captain's own framing of the two engine lists | `web/ui/src/app/screens/tech/view.rs`, `web/ui/src/app/screens/tech/view/assembly.rs` |
| Ship it to production | `docs/agent/COCKPIT-SMOKE-TEST.md` §HOW A CHANGE REACHES PRODUCTION (prebuilt on this Mac, needs Node 24) | `scripts/vercel-build-prod.sh`, `scripts/vercel-deploy-prod.sh` |
| Read the live Forge facts | the read model and the service boundary his lane finished | `db/src`, `forge/src/roles/`, `web/src/api/` |
| Know which gates exist and which are asleep | `.github/workflows/gates.yml` — fmt `:334`, secrets `:165`, the parked DB jobs `:443`, `:485`, `:518` | — |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `1a683113` | the Live selection is read before the message moves — the `E0505` that killed his branch's `rust core` before a single test ran | `RUSTFLAGS="-D warnings" cargo check -p ui -p db -p apis -p model -p services -p workflow --all-targets` → exit 0 (CI's own command) |
| `e7259f76` | `cargo fmt --all` over the tree, the four deferred trees untouched | the fmt step's exact grep (`.github/workflows/gates.yml:334-345`) → nothing outside the deferred trees |
| `f58d8db8` | the three fixture keys renamed to say what they are | `gitleaks git .` → no leaks; `cargo nextest run --profile ci` → 1062/1062 |
| `0a9623fb` | `middle/workflow/src/concurrency.rs` loses the two `store` imports its test never used — main's own warning debt, which `rust warnings are errors` would have failed | same `cargo check` → exit 0 |
| `b2d1da54` | `web/ui/src/app/screens/tech/view/assembly.rs` loses `result_runs`, orphaned by his own live-ops rework | same `cargo check` → exit 0; `cargo check -p ui --features wasm --target wasm32-unknown-unknown` → exit 0 |
| `a75859c9` | `rust/Cargo.lock` gains `async-trait` and `service` under the `forge` package — `47ea7977` (another lane, already on `main`) added both to `forge/Cargo.toml` and left the lock behind, so **rule 5's hook was refusing every push from every worktree**, and the deploy's `--locked` build would have failed | the hook's own test, `cargo metadata --manifest-path Cargo.toml --locked --offline` → exit 0 (exit 1 before this commit) |
| his 19 (`513f4afd`..`5f5fc052`) | the Forge hop: domain + db read model, the authorized live query, the service boundary and its API error mapping, Cockpit tabs named by purpose, Work in Flight wired to live data | every gate above ran on the **combined** stack — the first tree in which his work ever compiled. On `main` it is now gated by `static gates` and `rust core` like everything else |

His five failures, for the record: the `E0505` above; a 50-file rustfmt backlog (his rework **and** main's 2026-09-30
work); the three fixture keys; the tests downstream of the compile error; and `rust warnings are errors` /
`rust browser compile`, which never ran at all — the first is green on this tree by the import fix, the second by the
wasm check.

One receipt caveat, stated because the shape of this file is the point: `a75859c9`'s message names only the lock repair, and the same commit also carries the two corrections above (this table's lock row and H1's last sentence). One commit, two subjects, because an `--amend` folded them; the diff is the truth and the published history was not rewritten to fix a message.

## 5. NOT VERIFIED — the honest gaps

- **The three DB gates have never run and cannot run yet.** `rust DEV DB smoke` (`gates.yml:443`) and `rust DEV server
  boot + query` (`:485`) require `vars.RUST_DB_CI == 'true'` and a `DATABASE_URL_DEV` secret; `database-backed gates`
  (`:518`) requires `vars.FORGE_DB_CI == 'true'`. This repository reports **no variables and no secrets at all**
  (`gh variable list`, `gh secret list` → both empty), so all three are skipped on every ref, including a push to
  `main`. They are parked by design — their own comments call a credential-less gate that quietly passes "the failure
  this repository already recorded" — which means **the cockpit's read path has still never touched a real database**.
- **No deploy.** `scripts/vercel-build-prod.sh` and `scripts/vercel-deploy-prod.sh` were not run; nothing is in
  production. Production still serves the pre-stack build, so `COCKPIT-SMOKE-TEST.md` STEP 1's corner sha will **not**
  show this stack until someone builds and deploys.
- **No browser.** No screen was clicked and the Yew app was never served; the Cockpit changes were read and compiled
  (wasm `cargo check`), not seen.
- **`1062/1062` is the workspace suite**, which needs no Postgres. It is not a DEV-runtime proof — see the first bullet.
- **The unattended worker fast-forwards `origin/main` every 3 minutes** and works from whatever it finds, so this stack
  is what it runs from the first pass after the push. That is the design, not a bug, but it means the engine's new
  roles/engine code meets a real database before any DB gate has ever run over it.
- The four deferred trees are byte-identical to `b01f4692` — verified by diff — so the fmt commit did not touch them.
  The fmt gate re-checks this itself; do not "finish" the job by formatting them.
- **`main` is red on two backlogs this stack revealed, and caused neither.** At `b01f4692` the failing steps were
  `rust format` and `secrets — gitleaks`, both of which come *before* `dependencies — osv-scanner`
  (`.github/workflows/gates.yml:175`) and `rust file boundedness (800-line rule)` (`:403`) — so on `main` neither of
  the latter had run since before this work. With those two now fixed, both reached their steps and both failed:
  - `osv-scanner`: `56 advisories; 48 triaged` — 8 untriaged, **all npm**, none Rust: `brace-expansion@5.0.6`
    (`GHSA-6j4f-fj2g-mc7p`, `GHSA-q2hr-2g5m-vwhr`, `GHSA-qhr7-859c-m2p7`), `fast-uri@3.1.2` (`GHSA-hrr3-gc8f-f4qj`),
    `hono@4.12.25` (`GHSA-hxh3-vqpv-xpqv`), `ip-address@10.2.0` (`GHSA-h3mg-xc3c-68pw`, `GHSA-j6r3-76f7-8jcv`),
    `next@16.3.0` (`GHSA-vcvr-r3jv-pc5j`). The ledger's doctrine is that a triage row is a judgement, so no agent
    should add rows to make the step pass.
  - `rust file boundedness`: 12 `.rs` files over 800 lines, and the step has **no baseline** — it fails on any of them
    outside the four deferred trees, so it cannot be satisfied by fixing what this stack touched. Seven are untouched
    by anyone here (`web/ui/src/flight_recorder.rs` 1996, `db/src/forge_engine.rs` 2205,
    `web/ui/src/app/screens/flight_recorder.rs` 1366, `db/src/pool.rs` 970,
    `middle/workflow/src/neon/new_id.rs` 900,
    `rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` 902,
    `rust/test-harness/tests/wf_join__002__optional_siblings_handled_correctly.rs` 856). Four more were already over
    before this stack and stayed over, two of them shrinking (`web/src/luxesign/mod.rs` 1184→1170,
    `web/src/command_runtime.rs` 1447→1430, `web/src/signature/mod.rs` 932→935,
    `web/src/signer/mod.rs` 1145→1151). The policy this step does not implement is stated in
    `docs/agent/UI-SCREEN-ARCHITECTURE.md:301`: such a file "is split the next time it is edited".
  - **One of them is this stack's, and it is a collision between two gates**: `db/src/signature/database.rs`
    is 798 lines unformatted and **801** once rustfmt-clean (798 at his tip too — the fmt commit alone crossed it, by
    re-wrapping an `if` and a `map_err`). Formatted it breaks the 800-line step; unformatted it breaks the fmt step.
    Only a real move-only split satisfies both, and doing it while 7 untouched files keep the step red would buy
    nothing but risk.
  - The three DB jobs again reported `skipped` on `main` — the parked state of §5's first bullet, observed rather
    than assumed.

## 6. OPEN — the next actions, in order

1. **Read the push result — it is already in.** `gates` at `8e2794ab`: `static gates` ✗ at `dependencies — osv-scanner`,
   `rust core` ✗ at `rust file boundedness (800-line rule)`, the three DB jobs `skipped`. Everything this stack owned
   is green in that list — `rust format`, `secrets — gitleaks`, the workspace check, the test suite, the wasm compile.
   The two ✗ are §5 items, not this stack's. Finished; do not re-derive it.
2. Decide the 800-line rule (§7 next). Until it is decided, `rust core` is red for every push by anyone, whatever they
   change, and the friction grows with each lane's next edit.
3. Decide the advisory triage (§7). `static gates` is red for everyone until the ledger or the versions change.
4. Arm the parked DB gates — captain only: `gh secret set DATABASE_URL_DEV --body "<the DEV Neon URL>"`,
   `gh variable set RUST_DB_CI --body true`, `gh variable set FORGE_DB_CI --body true`. Finished when a push to `main`
   shows the three jobs *running*; the cockpit read path then gets its first real DEV read.
5. Run the captain's script on the built app (`docs/agent/COCKPIT-SMOKE-TEST.md`), checking the corner sha first.
   Finished when STEP 2 (`clear bench`) behaves as written.
6. Delete `origin/forge-service-finish-20261002` in the GitHub UI. Finished when the repository has no branch but
   `main`, which is what rule 1 asks for.

## 7. ASK THE OWNER

- **The 800-line rule — split the 12 files, or give the step the baseline the policy describes?** Split → a move-only
  programme across `web`, `db`, `web/ui` and `rust/test-harness`, i.e. three lanes' code, and
  `rust core` stays red until the last file is done. Baseline → the step compares against the files that were already
  over at a named commit and lets the rule bite on the next edit, which is what `UI-SCREEN-ARCHITECTURE.md:301` already
  says. Nothing else turns `rust core` green.
- **The 8 npm advisories — triage them or bump the packages?** Triage → eight rows in
  `docs/agent/DEPENDENCY-TRIAGE.md`, each with a reachability judgement, because the ledger is the single writer of
  that fact and an agent adding rows to make the step pass is the anti-pattern its own header warns about. Bump →
  `brace-expansion`, `fast-uri`, `hono`, `ip-address` and `next` in `pnpm-lock.yaml`, plus whatever the bump drags in.
  Nothing else turns `static gates` green.
- **Arm the DB gates?** Yes → the three skipped jobs start running on `main` and the read path is finally proven
  against DEV. No → they stay visibly skipped, and no agent may claim the cockpit's data path works.
- **Delete `origin/forge-service-finish-20261002`?** Yes → done in the UI, rule 1 restored. No → it stays as the record
  of a branch that should not have existed.
- **Rebuild production now?** Yes → `scripts/vercel-build-prod.sh` then `scripts/vercel-deploy-prod.sh` (Node 24 first)
  and the screens are live. No → the stack sits in git, production serves the old build.

