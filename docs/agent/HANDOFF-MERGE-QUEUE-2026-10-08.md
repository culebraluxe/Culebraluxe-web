# Hand-off — the land queue: what is on `main`, what is not, and why the merge is blocked

Written 2026-10-08 from `lane/deep`, read-only against `origin` (no lane was touched, nothing merged).
Sections 4 and 5 were measured at `main` = `47fce4b50`. **The queue moved while this was written** — see
the re-check at the foot of section 1. Re-run the commands; do not trust this page's numbers without them.

## 1. The overnight table says "landed"; "✅ resolves" only proves the SHA exists

"✅ resolves" was checked with `git cat-file -e <sha>` — that proves the SHA exists in the repo, not that
`main` has it. Asked the other question (`git merge-base --is-ancestor <sha> origin/main`), at `47fce4b50`:

    ON main:      49f1ab1f0 (FIX-001), cb766b80 (FIX-003)          — via 47fce4b50, the muse-2 merge
    NOT on main:  015ac058 (FIX-002) a883892c (FIX-007)            — lane/nemotron   (22 ahead)
                  24d032c9 (FIX-004) bd1b093f5 (FIX-005) f4586f5dc (FIX-010) — lane/muse (10 ahead)
                  32832c57 (FIX-006)                               — lane/nemotron-2 (7 ahead)
                  b19c1fd3 (FIX-008)                               — lane/fledge     (5 ahead, its tip)

Eight of ten were delivered but unpulled, hostage to the same merge that was blocked.

**Re-checked minutes later at `d080c968`** (23 commits on, the nemotron merge): `015ac058` and `a883892c`
are now ON main, so **four of ten** are. Still out: FIX-004/005/010 on `lane/muse`, FIX-006 on
`lane/nemotron-2`, FIX-008 on `lane/fledge`. The merge queue is moving; the shape below is unchanged.

## 2. The blocked merge is not reproducible from any remote tip

`origin/lane/muse-2`'s tip is `cb766b80`, which is *already on main*. No remote branch conflicts on all
three files the coordinator named: `origin/lane/spacebunny` conflicts on `tests/src/lib.rs` and
`tests/src/security.rs` but not on `web/src/api/google_auth.rs`; `origin/lane/muse` and
`origin/lane/nemotron-2` conflict on `google_auth.rs` but neither on the harness. The attempted merge
therefore mixed a **local, unpushed** branch state. Rule 2 ("push after every commit") is what makes a
blocked merge diagnosable; without it nobody can reproduce the conflict — push the branch first.

## 3. The `SecurityHarness` collision is a rename, not a union

Two files named `tests/src/security.rs` that share a struct name and nothing else:

    origin/main  153 lines  "the security-audit write boundary" — L2, wraps SecurityAuditDao + TestDatabase
    spacebunny   363 lines  "the SEC.IDENTITY / SEC.ENTITLEMENT harness" — real SecurityService + Casbin,
                            in-memory adapters, no socket

Not two versions of one harness: two unrelated harnesses with colliding names. "Unifying" them by name
would produce a file that is neither, so a coordinator should not make that call. The author renames its
own module (`security_identity.rs`, struct `IdentitySecurityHarness`), unions the one-line export in
`tests/src/lib.rs`, and takes main's newer `web/src/api/google_auth.rs` (the CR/LF fix) — a rebase puts
them in exactly that position. **Recommendation: REBASE, not RESOLVE**, which is also the lane rule
("a lane syncs by rebasing onto `origin/main`, never by merging `main` in").

## 4. Per-lane conflict surface (`git merge-tree --write-tree --name-only origin/main <ref>`)

    lane/mimo                  2 ahead   0 conflicts
    lane/muse-2-batch19        2 ahead   0 conflicts
    lane/fledge                5 ahead   3   tests/src/lib.rs · docs_forms_execution__007 · docs_forms_template__001
    lane/spacebunny-batch-39   4 ahead   1   tests/Cargo.toml
    lane/nemotron              22 ahead  5   Cargo.lock · docs/agent/MEMORY.md · forge_launch_intent__001/002 · ui_model__007
    lane/muse                  10 ahead  5   forge/src/engine/maestro.rs · opencode.rs · crm_person__002 · sec_redirect__011 · google_auth.rs
    lane/nemotron-2            7 ahead   6   sec_entitlement__008 · sec_redirect__007/008/009/010 · google_auth.rs
    lane/spacebunny            30 ahead  5   HANDOFF-TST-REDIRECT-2026-10-08.md · MEMORY.md · tests/Cargo.toml · tests/src/lib.rs · tests/src/security.rs
    lane/longcat               5 ahead   6   docs_forms_template__005/006/007 · docs_vault__001/002 · docs_vault__005
    lane/nemotron-lightning    75 ahead  24  tests/Cargo.toml · tests/src/crm.rs · mvi.rs · 6 × crm_* · web/src/api/routes.rs
                                             · web/src/document_sign/mod.rs · web/src/signer/mod.rs
                                             · web/ui/src/app/screens/forms/editor.rs · sign_document.rs …

Order that costs least: the four fix-carrying lanes first (fledge, muse, nemotron, nemotron-2), then
spacebunny, and the 75-commit lane last and **in per-batch pieces** — a whole-lane rebase of it is a day.

## 5. Three fixes can land today as patches, four cannot

`git format-patch -1 --stdout <sha> | git apply --check -` against `main` (the fix's own delta, not the
lane's whole tree):

    applies cleanly:  24d032c9 (FIX-004) · 32832c57 (FIX-006) · f4586f5dc (FIX-010)
    needs context:    015ac058 (writer.rs) · bd1b093f5 (maestro.rs) · a883892c (Cargo.toml) · b19c1fd3 (a test file)

Cherry-picking the three makes new SHAs and re-runs their receipts — allowed, since the code is the asset
and the SHA is a label — but it is a Captain's call, and no lane may commit another lane's work.

## 6. The red guard on trunk has half a fix sitting unlanded

`tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs` fails on trunk twice over. Both
mechanisms were re-read on `origin/main` `d080c968` and are intact — the test itself was last *run* at
`47fce4b50`, so treat this as a reading, not a fresh red:

- line 910 — `cli/src/forge/lint.rs:105-108`, where `430a13761` ("rustfmt drift", 2026-10-07) wrapped the
  one-line `pattern!` and the test asserts the one-line spelling. `lane/longcat` `3b32cc4ff` (batch 28)
  already fixes this **honestly** — it asserts the regex the lint must name, not its line breaks — but the
  commit is unlanded and its base predates the second break.
- line 880 — the guard's own file sweep (`arch_boundary__011__qa_cannot_own_git_mutations.rs:880`) counts
  `tests/tests/forge_assay__005__qa_cannot_modify_git.rs:32-34`
  (a test that *tests* that QA cannot push, which necessarily spells `push`/`merge`/`rebase`) beside
  `forge/src/engine/git_publish.rs`, and refuses. This one needs the sweep's owner: production code is what
  the sweep is for; excluding `tests/` from a sweep about production mutation paths is a scope decision,
  and it is not a baseline edit — but it must be argued and owned, not silent.

## 7. Batch 38 is in no commit, on any ref

`git log --all --oneline --grep='batch 38' -i` → nothing. `origin/lane/longcat` carries batches 8, 11,
21, 25, 28 and its `wip/lane/longcat` snapshot tops out at the same commit. So a quiet lane is not always
a lane resting: **38's work is not pullable from anywhere**, which by rule 6 means it does not exist yet.
Ask its author to push the branch; do not count it as terminal on the strength of a status table.

## 8. Not verified here

- Nothing was run against DEV or PROD: this page is git evidence only.
- No lane was checked out, rebased, merged or built; the conflict lists are `merge-tree` output, not
  resolutions.
- The three "applies cleanly" patches were tested with `git apply --check` and **not** applied or built.
- Batch 38's absence is git-wide; a record of the work could still live outside git (a DB row, an unpushed
  doc) — I did not search there.
- `lane/deep`'s own work is unaffected and unchanged: this page is analysis, not a merge.
