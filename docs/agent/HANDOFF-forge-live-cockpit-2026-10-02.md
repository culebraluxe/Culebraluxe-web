# HANDOFF — the Forge live cockpit, landed on main (2026-10-02)

Written by the next agent, because the session that built this stopped without writing one. It crashed mid-work and
left 19 commits on `forge-service-finish-20261002` — a branch the House Rules forbid — while `origin/main` sat at
`b01f4692`. By rule 1 the work did not exist to anyone else, and by rule 8 it was not done: the branch's `rust core`
died on a compile error before any test ran. Those 19 commits are on `main` now, with five cleanup commits on top,
and this file travels with the same push.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | The 19 commits (`513f4afd`..`5f5fc052`) and five cleanup commits — `1a683113` (the Live selection), `e7259f76` (rustfmt), `f58d8db8` (fixture rename), `0a9623fb` (workflow imports), `b2d1da54` (the orphan `result_runs`) — plus this file are on `main`, fast-forwarded from `b01f4692`: no merge commit, no branch left behind | `git log --oneline origin/main \| head -26` |
| S2 | `main` was red **before** this stack: `gates` failed at `b01f4692` (2026-10-02 11:27) on the pending rustfmt backlog and on three fixture keys gitleaks read as secrets. Neither is the cockpit's fault and both are fixed here | `gh run list --branch main --limit 3` |
| S3 | The stack is 58 files and **every one is a modification** — no file is added: 10 in `rust/forge/src/roles`, 5 in `rust/test-harness/tests`, 5 in `rust/core/domain/src`, 4 in `rust/server/src`, 4 in `rust/forge/src/engine`, 4 in `rust/core/db/src`, 7 under `rust/ui/src/app/screens/tech`, 2 in `rust/core/workflow/src`, and singles | `git diff --name-only b01f4692..origin/main \| wc -l` |
| S4 | The three fixture keys now say what they are instead of being excused: `definition-kinds-under-test-not-a-secret`, `definition-self-loop-under-test-not-a-secret`, `definition-orphan-under-test-not-a-secret` | `rust/test-harness/tests/wf_definition__005__missing_target.rs:64,86,221` |
| S5 | Only `VALID_DEFINITION` (`:51`) is asserted; the three renamed keys are parsed and never compared, so the rename weakens no test | `rust/test-harness/tests/wf_definition__005__missing_target.rs:113,144` |
| S6 | `.gitleaksignore` gained three fingerprints, naming the two commits that carried the old literals, each with its reason written beside it. The file's own doctrine — "rename the fixture, not a line here" — is repeated in that block | `.gitleaksignore` (the 2026-10-02 block) |
| S7 | The Cockpit tabs are named by operating purpose, Work in Flight is wired to Forge live data, and the live metric values are borrowed rather than cloned | `rust/ui/src/app/screens/tech/view.rs`, `rust/ui/src/app/screens/tech/view/assembly.rs` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | Three uncommitted files in `/Users/lisapenfieldicloud.com/Documents/Culebraluxe-web`: `rust/Cargo.lock` (modified), `db/loads/arm_recovery_batch_2026_10_01.sql` and `db/loads/settle_landed_candidates_2026_10_01.sql` (untracked) | the live lane working in that folder | do not commit, stash, clean or reset them. That lane's next `git pull --rebase` will complain about the modified lock; clearing it is their call, not yours |
| H2 | The four deferred trees: `rust/cli/src/apple_mail`, `rust/cli/src/forge/lint` (`lint.rs` **and** `lint/`), `rust/core/domain/src/applemail`, `rust/core/domain/src/apple_messages` | the captain — the fmt gate defers them by name (`.github/workflows/gates.yml:338`) | never run `cargo fmt --all` across them and never hand-format them. `rust/cli/src/forge/lint.rs:103` defines `pattern!(roles_that_may_not_commit, r"(?i)\b(scout\|assay\|inspector)\b")` as **source text**, and `rust/test-harness/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:193` pins that exact string: rustfmt reformats it and the assay fails. Hit this while cleaning up; reverted |
| H3 | `origin/forge-service-finish-20261002` and PR #35 | the captain | never push to it — the pre-push hook refuses any branch but `main`. Its commits are on `main` now, so PR #35 should read as merged; deleting the remote branch is a GitHub-UI job |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Change what the Forge live cockpit shows | `docs/agent/COCKPIT-PURPOSE.md` — the captain's own framing of the two engine lists | `rust/ui/src/app/screens/tech/view.rs`, `rust/ui/src/app/screens/tech/view/assembly.rs` |
| Ship it to production | `docs/agent/COCKPIT-SMOKE-TEST.md` §HOW A CHANGE REACHES PRODUCTION (prebuilt on this Mac, needs Node 24) | `scripts/vercel-build-prod.sh`, `scripts/vercel-deploy-prod.sh` |
| Read the live Forge facts | the read model and the service boundary his lane finished | `rust/core/db/src`, `rust/forge/src/roles/`, `rust/server/src/api/` |
| Know which gates exist and which are asleep | `.github/workflows/gates.yml` — fmt `:334`, secrets `:165`, the parked DB jobs `:443`, `:485`, `:518` | — |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `1a683113` | the Live selection is read before the message moves — the `E0505` that killed his branch's `rust core` before a single test ran | `RUSTFLAGS="-D warnings" cargo check -p ui -p db -p integrations -p domain -p service -p workflow --all-targets` → exit 0 (CI's own command) |
| `e7259f76` | `cargo fmt --all` over the tree, the four deferred trees untouched | the fmt step's exact grep (`.github/workflows/gates.yml:334-345`) → nothing outside the deferred trees |
| `f58d8db8` | the three fixture keys renamed to say what they are | `gitleaks git .` → no leaks; `cargo nextest run --profile ci` → 1062/1062 |
| `0a9623fb` | `rust/core/workflow/src/concurrency.rs` loses the two `store` imports its test never used — main's own warning debt, which `rust warnings are errors` would have failed | same `cargo check` → exit 0 |
| `b2d1da54` | `rust/ui/src/app/screens/tech/view/assembly.rs` loses `result_runs`, orphaned by his own live-ops rework | same `cargo check` → exit 0; `cargo check -p ui --features wasm --target wasm32-unknown-unknown` → exit 0 |
| his 19 (`513f4afd`..`5f5fc052`) | the Forge hop: domain + db read model, the authorized live query, the service boundary and its API error mapping, Cockpit tabs named by purpose, Work in Flight wired to live data | every gate above ran on the **combined** stack — the first tree in which his work ever compiled. On `main` it is now gated by `static gates` and `rust core` like everything else |

His five failures, for the record: the `E0505` above; a 50-file rustfmt backlog (his rework **and** main's 2026-09-30
work); the three fixture keys; the tests downstream of the compile error; and `rust warnings are errors` /
`rust browser compile`, which never ran at all — the first is green on this tree by the import fix, the second by the
wasm check.

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

## 6. OPEN — the next actions, in order

1. Watch the push: `gh run list --branch main --limit 1`, then `gh run view <id>`. Expect `static gates` and `rust core`
   to pass (both were run locally with CI's exact commands on this exact tree) and the three DB jobs to report
   **skipped**. Finished when the run is green and the three skips are read in the log, not assumed.
2. Arm the parked DB gates — captain only: `gh secret set DATABASE_URL_DEV --body "<the DEV Neon URL>"`,
   `gh variable set RUST_DB_CI --body true`, `gh variable set FORGE_DB_CI --body true`. Finished when a push to `main`
   shows the three jobs *running*; the cockpit read path then gets its first real DEV read.
3. Run the captain's script on the built app (`docs/agent/COCKPIT-SMOKE-TEST.md`), checking the corner sha first.
   Finished when STEP 2 (`clear bench`) behaves as written.
4. Delete `origin/forge-service-finish-20261002` in the GitHub UI. Finished when the repository has no branch but
   `main`, which is what rule 1 asks for.

## 7. ASK THE OWNER

- **Arm the DB gates?** Yes → the three skipped jobs start running on `main` and the read path is finally proven
  against DEV. No → they stay visibly skipped, and no agent may claim the cockpit's data path works.
- **Delete `origin/forge-service-finish-20261002`?** Yes → done in the UI, rule 1 restored. No → it stays as the record
  of a branch that should not have existed.
- **Rebuild production now?** Yes → `scripts/vercel-build-prod.sh` then `scripts/vercel-deploy-prod.sh` (Node 24 first)
  and the screens are live. No → the stack sits in git, production serves the old build.

