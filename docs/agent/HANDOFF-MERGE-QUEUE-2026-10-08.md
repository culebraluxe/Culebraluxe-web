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

## 9. UPDATE, later the same day: nine of ten fixes are on `main`, and §1's counts are superseded

While this page was being acted on, the coordinator merged four more fixes, each as its own merge on `main`:
`530903627` (muse: FIX-004 `24d032c9` + FIX-005 `bd1b093f5`), `1a0f60269` (nemotron-2: FIX-006 `32832c57`),
`8a3b7f950` (fledge: FIX-008 `b19c1fd3`). Ancestry re-checked with `git merge-base --is-ancestor` against
`origin/main`: `49f1ab1f0`, `015ac0581`, `cb766b802`, `24d032c98`, `bd1b093f5`, `32832c57c`, `a883892c1`,
`b19c1fd3c`, `f4586f5dc` — **all nine ON MAIN**.

**FIX-009 does not exist.** `git log --all --grep='FIX-009'` is empty on every ref, so it was never authored
anywhere: it is not "still out", it was never delivered. That needs a commission, not a merge.

The lane's own contribution ended up being the one thing nobody else had done — §9.1 — plus the schema
obligation in §9.2. FIX-004, FIX-005, FIX-006 and FIX-010 were carried here as cherry-picks and then dropped
by the rebase, each landing with `patch contents already upstream`.

### 9.1 `abe6513e5` — the call sites three API changes moved, and nothing else

`main` was red at T0 for 16 test targets because three API changes had landed without their callers:
`execute_claimed_job_unsettled` (FIX-006: +interrupt handle, +turn ceiling), `finish_agent_work_run`
(FIX-008: +caller idempotency key) and `heartbeat_agent_work` (FIX-008: owner-fenced, +worker_id +lease_ttl).
`abe6513e5` repairs 21 files (+30/-30): `None, None` for the interrupt handle and the ceiling-derived
supervisor deadline, `None` for the settlement key, and the owner the test itself claimed with for the
heartbeat — any other id is refused by the owner fence. Same finding as FIX-006's lane: it changed the
signature and left its own callers on the old form.

`cargo check --workspace --all-targets --keep-going` on the pushed tree: **3 red targets, none from this
work** — `docs_vault__007__pdf_byte_handling`, `sec_redirect__003__host_injection`,
`sec_redirect__005__fragment_handling`. Every file behind those errors is byte-identical to trunk
(`web/src/api/google_auth.rs` has `origin`/`redirect_uri` private while the tests import them;
`db/src/signer.rs` grew `Certificate` / `FinalizeRecipient.completed_at` / `FinalizeEvent.recipient_id` after
the test was written). Unowned trunk red — and the reason the push used `CULEBRALUXE_SKIP_BUILD_CHECK=1`,
named here as rule 9 requires.

### 9.2 The one schema obligation was paid, DEV and PROD

FIX-008's `db/migrations/277_forge_agent_work_lease.sql` was applied to **PROD** through the recorded path
(`cargo run -p cli -- db-tool apply … prod`): ledger row `2026-10-08 18:54:50`, checksum
`sha256:61ce9891ae47e523b92ae39f4a5ec2798727b354848a3d61e1f5e6ff993bc89a` — the digest DEV already held, and
the same bytes `main` carries. `db-tool parity` after it: column drift 0, fk drift 0, check drift 0; the only
drift is two DEV-only tables (`chaos_service_test_writes`, `crm_lead_projection`) with their pkeys, both
pre-existing. 277 is additive apart from two `drop function if exists` lines that retire the **old
signatures** of `forge_finish_agent_work_run` and `forge_record_tool_artifact` — old code and the new schema
are mutually exclusive by construction, which is why code and migration had to land together. PROD was idle
at the cutover (0 live executions, 0 `Claimed`/`Running` items, nothing touched in the previous hour).

### 9.3 What was run (T1, DEV, on the pushed tree)

- `cargo test -p forge --lib` → **382 passed, 0 failed, 2 ignored**.
- `forge_queue__001__fenced_claim`, `db_concurrency__007__stale_claim_vs_live_heartbeat`,
  `forge_receipt__001__template_rejected`, `forge_fleet__001__nine_way_split`, `forge_claim__004`,
  `forge_claim__005`, `forge_job__014` (`--include-ignored`) → **15 passed, 0 failed**.

### 9.4 Still open after this landing

- **FIX-009** (quarantine/requeue): authored nowhere; needs a decision.
- The **3 red targets** in §9.1: `sec_redirect__003`/`__005` are a visibility change in
  `web/src/api/google_auth.rs` (or a test that goes through the public route); `docs_vault__007` needs the
  certificate fixture rewritten against the current `signer.rs`. Both belong to their stories, not to this
  queue.
- §3 (the harness rename), §6 (the line-880 sweep scope) and §7 (batch 38) are unchanged — and worth noting:
  **no fix ever needed the harness resolved.** All nine landed without touching it.

### 9.5 Not verified here

- The 3 red targets were not repaired, only named; the exact fixture values `docs_vault__007` needs were not
  designed.
- No full-suite run (T2): only the crates and targets named in §9.3.
- `forge:clean` was deliberately **not** run (production-mutating, needs the Captain's go); the DB-backed
  tests were run against the DEV target their harness asserts.

---

## 10. The three red targets in §9.1 are repaired and landed — `lane/deep`, 2026-10-08

The Captain directed the two repairs in §9.4 to this lane. They landed on `main` as three commits and
`origin/main` moved `7b93c3e5a..23a950ca3`. None of the three needed a fix from §3–§7, and the harness
rename was still not required.

1. **`abdc89512` — `docs_vault__007__pdf_byte_handling`.** Main's copy was the stale call: a six-argument
   `render_completion_certificate` where `web/src/vault/signing_certificate.rs:279` now takes
   `&Certificate<'_>`, plus a missing `FinalizeEvent.recipient_id` (`db/src/signer.rs:81`) and
   `FinalizeRecipient.completed_at` (`:56`). This is **not a rewrite**: it is a cherry-pick of Muse's
   already-authored `b95adc28c` on `lane/muse`, authorship preserved, resolved as the add/add conflict it
   was (his lane branched before main's copy landed) in favour of his rebuild. That his version is the
   current one was verified mechanically, not by eye: `git diff --cached b95adc28c -- <path>` is empty, so
   the landed file is byte-identical to his commit.
2. **`0a88d98f0` — the redirect seam.** `origin` and `redirect_uri` are now `pub`. The brief circulating for
   this work said `pub(crate)`, and **`pub(crate)` cannot work**: `TST-SEC-REDIRECT-003`/`-005` live in
   `tests/tests/`, which is the separate `test-harness` crate (`use web::api::google_auth::{origin,
   redirect_uri}`), and `pub(crate)` is invisible outside `web` — that change would have reproduced
   `E0603`. The file's own precedent agrees: `pub fn safe_next` (`:82`) and `pub fn percent_decode` (`:239`)
   are public in this file for exactly these contract tests. Both functions are pure reads of request
   headers; each carries a doc comment naming the consuming tests.
3. **`23a950ca3` — the defect the seam exposed.** With the target compiling for the first time, `003` ran
   and failed on one assertion: `origin(&[("host", "[::1]:3000")])` returned `https://[::1]:3000` where the
   contract test asserts `http://[::1]:3000`. The predicate recognised only `localhost` and `127.0.0.1` as
   loopback, so an IPv6 loopback dev origin was handed `https` while the browser sat on `http` — a
   redirect-URI mismatch in local dev. One line added: `host.starts_with("[::1]")`. A public IPv6 literal
   (`[2001:db8::1]`) still resolves to `https`, which the same test also asserts. Neither the story's
   proposal patch (`docs/agent/proposals/TST-REDIRECT-2026-10-08.patch`, which is about control characters)
   nor `main` carried this fix, so the gap was unowned.

**Do not redo this work.** `lane/fledge` was briefed on the same seam and has nothing pushed on it
(`lane/fledge` still sits at `b19c1fd3c`, and no ref anywhere carries a seam change); a second copy of
`web/src/api/google_auth.rs` landing after this one is the collision §2 of this queue was written to stop.

### 10.1 Verification on the pushed content

- Target build (this is T0 for the change): `cargo check -p test-harness --test docs_vault__007__pdf_byte_handling
  --test sec_redirect__003__host_injection --test sec_redirect__005__fragment_handling` → `exit=0`, 0 errors.
  The only warning in those targets is `non_snake_case` on the test fn names, which is this suite's own
  convention (116 occurrences across the workspace).
- The tests themselves: `cargo test -p test-harness --no-fail-fast` over the same three targets →
  `docs_vault__007 ... ok`, `sec_redirect__003 ... ok`, `sec_redirect__005 ... ok`, three ×
  `1 passed; 0 failed`, `exit=0`.
- The crate behind the changed file: `cargo test -p web google_auth` → `10 passed; 0 failed`, `exit=0`.
- Workspace T0: `cargo check --workspace --all-targets --keep-going` → `EXIT=0`, the first fully green
  workspace check of this landing (the earlier push today carried
  `CULEBRALUXE_SKIP_BUILD_CHECK=1` because of these three targets; **this push used no skip flag and the
  hook did not refuse it**).

### 10.2 Still open

- **FIX-009** (§9.4): unchanged — authored nowhere, awaiting the Captain's commission or drop.
- **rustfmt drift** (~150 files: `crm_*`, `wf_*`, `ui_*`, `forge_assay_*`): pre-existing, wants one quiet-trunk commit.
- **`forge/src/engine/executor/drive.rs:303`**: `unsafe { std::mem::transmute(harness) }` to
  `&'static dyn RoleHarness` inside a closure that outlives its `Arc` — a latent soundness bug, not repaired here.
- **Not verified here**: no T2 full-suite run (that stays CI's, per the tiered rule), and no `pnpm slice:check`.

