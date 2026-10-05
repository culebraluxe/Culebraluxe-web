# Handoff — estate-wide lane rescue, captain-directed (2026-10-05)

Captain's order: "check all lanes commit and push them all to main." Nine lanes were landed. Four files were **not**
landed and are held, with their content preserved three ways. Nobody ran a compile or a test in this operation.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | Nine lane work-streams are on `origin/main`; main tip `59b55c9d`. | `git log --oneline -10 origin/main` |
| S2 | No lane is ahead of main any more: `ahead=0` for all 14 lanes. | `for d in /Users/Shared/dev/src/lane-*; do git -C $d rev-list --count origin/main..HEAD; done` |
| S3 | The four disputed files are held, not landed, not lost: blob ids `0b5ad0ab` (muse 004 draft), `c8690533` (muse 006), `c285619c` (mimo 006), `e6d8f2e4` (muse-2 006). | `git hash-object /Users/Shared/dev/build/held/2026-10-05/*.rs` |
| S4 | Held copies exist on disk outside every repo, in the shared build area. | `ls -lh /Users/Shared/dev/build/held/2026-10-05/` |
| S5 | muse's two held blobs are still reachable from the WIP snapshot ref `refs/wip/lane-muse` (`5d6506ea`); mimo's and muse-2's from their pushed lane branches. | `git show refs/wip/lane-muse --stat`; `git -C /Users/Shared/dev/src/lane-mimo show origin/lane/mimo:tests/tests/arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical.rs \| git hash-object --stdin` |
| S6 | `arch_route_map__006` was never on `main` — it is not an orphaned-from-main file, it is three independent drafts of the same path. | `git log origin/main --diff-filter=AD --name-status -- tests/tests/arch_route_map__006__no_retired_typescript_path_is_referenced_as_canonical.rs` (empty) |
| S7 | `arch_route_map__004` **is** canonical on main: added by `1d92b299`, 294 lines. lane-muse's copy is a 77-line draft of the same path. | `git log origin/main --diff-filter=A --name-status -- tests/tests/arch_route_map__004__no_route_silently_appears_without_classification.rs` |
| S8 | Two untracked workflow-scratch directories (`Working/`) and one new JS file (`eval_tst.js`) were deliberately not committed. | `git -C /Users/Shared/dev/src/lane-muse-2 status --porcelain`; `git -C /Users/Shared/dev/src/lane-nemotron-2 status --porcelain` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | `arch_route_map__006` — three divergent drafts (mimo 475 lines, muse-2 180, muse 119), no writer. | The Captain | Do not land any of them; do not pick a winner. One owner, one file (AGENTS.md "Let two sources answer one fact"). |
| H2 | lane-muse's `arch_route_map__004` draft (77 lines) vs main's canonical 294-line test. | The Captain | Main is the writer; the draft is superseded. Land it only if the Captain rules it an improvement. |
| H3 | `Working/` (lane-muse-2, lane-nemotron-2) — untracked workflow-artifact JSON (~9k lines). | The Captain | Not versioned: "NO TREES. EVER" (AGENTS.md). Left on disk; WIP snapshots keep them. |
| H4 | `eval_tst.js` (lane-nemotron-2) — new untracked JS. | The Captain | Not versioned: the TypeScript ratchet forbids new tracked JS outside `legacy/`. |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Decide the 004/006 owners | this file, §7 | the four files in `/Users/Shared/dev/build/held/2026-10-05/` |
| Verify what actually landed | `git log --oneline origin/main` | `web/src/vault/signing_overlay.rs`, `forge/src/engine/maestro.rs`, `tests/tests/*` |
| Run the gate this rescue owes | AGENTS.md "The gate is tiered" | `pnpm slice:check` (T1), CI `gates.yml` (T2) |


## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `9cca1b00` | lane-claude: Initiation playbooks Phase 01-06 | pre-push hook: `Cargo.lock --locked` only (no `web/ui`, and the workspace compile belongs to CI by design) |
| `fc88c061` | lane-deep: `forge/src/pianola/{escalation,status}.rs`, `db/src/forge_read.rs`, boundary arch test | same |
| `d21418ed` | lane-nemotron: crm_client 001-003 + crm_catchup 002-006 tests, `tests/src/crm.rs` wiring | same |
| `dccd2dd7` | lane-nemotron-lightning: db_schema 003-008, db_transaction 001-004 | same |
| `18cbadd3` | lane-nemotron-2: db_concurrency 006-013, `pg` bump | same |
| `35dade9a` | lane-spacebunny: CRM.PERSON 003-007 + DB.CONCURRENCY 001-005 | same |
| `5675c2a9` | lane-mimo: crm_comms 001-005 | same |
| `08cd23ad` `35490b12` `9d6a105d` `695733d4` `dd26b6be` | lane-muse-2: fail-closed harness selection, Maestro send contract, preflight, TST RED batch 6 (5 commits) | same |
| `59b55c9d` | lane-muse: document signing + vault signing overlay, playbooks, `Cargo.toml` + `Cargo.lock` together | same |

## 5. NOT VERIFIED — the honest gaps

- **Nothing was compiled or tested.** The pre-push hook deliberately does not run the workspace compile (it moved to
  CI on 2026-10-01); `CULEBRALUXE_SKIP_BUILD_CHECK` was **not** used, and no `cargo check`, `pnpm slice:check` or
  `cargo nextest` was run against any of these nine landings. Their T0/T1 is owed, not paid.
- Several landings are the authors' own "faithful RED" contract tests (TST RED batches 4 and 6): they are expected to
  fail at runtime by design. A red CI run on `main` from them is expected, not a regression introduced here.
- The `pg ^8.23.0 → ^8.23.1` bump in `package.json` was landed without a `pnpm install` or lock check.
- lane-deep's `forge/src/pianola/*` + `db/src/forge_read.rs` changes were not written by the agent that committed them:
  they were the lane's pre-existing uncommitted work, committed blind under the Captain's order. Their author should
  confirm the intent.

## 6. OPEN — the next actions, in order

1. Run the tier these landings owe: `pnpm slice:check` locally, and read the CI `gates.yml` run for `main` at `59b55c9d`.
   Finished when the changed sections are green or each failure is a named, dated, owned row.
2. Give `arch_route_map__006` exactly one owner (§7 Q1). Finished when one draft is landed and the other two are
   deleted by their lanes, or all three are dropped.
3. Rule on lane-muse's 004 draft (§7 Q2). Finished when it is either landed as an improvement to the canonical test or
   deleted.
4. Decide on `Working/` and `eval_tst.js` (§7 Q3). Finished when they are deleted or relocated outside the lanes.

## 7. ASK THE OWNER

- **Q1: who owns `arch_route_map__006`?** Answer `mimo`, `muse-2`, `muse`, or `none`. On a lane name I land that one
  copy from `/Users/Shared/dev/build/held/2026-10-05/` and delete the other two; on `none` I delete all three.
- **Q2: is lane-muse's 77-line `arch_route_map__004` an improvement on main's 294-line test?** Answer `yes` or `no`.
  On `no` it is deleted from the holding area; on `yes` it lands as a replacement and the canonical test's history is
  named in the commit message.
- **Q3: keep `Working/` and `eval_tst.js`?** Answer `delete` or `keep`. On `delete` they are removed from both lanes
  (WIP snapshots still hold them for one machine-lifetime); on `keep` they stay untracked.
