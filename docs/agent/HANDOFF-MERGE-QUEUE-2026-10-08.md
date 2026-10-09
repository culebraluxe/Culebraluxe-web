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

The Captain directed the two repairs in §9.4 to this lane. They landed on `main` as five commits and
`origin/main` moved `7b93c3e5a..9d12c571f`. None of the three needed a fix from §3–§7, and the harness
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
- FMT: `rustfmt --edition 2021 --check web/src/api/google_auth.rs` → `exit=0`, **after** a repair this
  slice needed: that same check exits 0 on the file at `7b93c3e5a` and exited 1 on it at `23a950ca3`, so
  the drift was this slice's own and `9d12c571f` fixes it. Recorded because the gate mislabelled it: see below.
- `pnpm slice:check` → `RESULT the slice may be handed over (T0 + FMT + T1 green; T2 belongs to CI)`,
  `EXIT=0`; T0 PASS (52s), FMT PASS (4s), T1 `sections to run: (none)`. Its **T1 selection is empty by
  construction** here — it measures the slice since its merge-base with `origin/main`, and by then this
  slice *was* `origin/main`, so it found no source file to select. The sections were therefore run by hand,
  above. For the same reason its FMT stage printed `web/src/api/google_auth.rs` among "pre-existing, NOT
  this slice", which was **wrong about my own drift** — a label that only the direct file-level check
  caught. A gate that measures a slice already on trunk reports green over its own content.

### 10.2 Still open

- **FIX-009** (§9.4): unchanged — authored nowhere, awaiting the Captain's commission or drop.
- **rustfmt drift** (~150 files: `crm_*`, `wf_*`, `ui_*`, `forge_assay_*`): pre-existing, wants one quiet-trunk commit.
- **`forge/src/engine/executor/drive.rs:303`**: `unsafe { std::mem::transmute(harness) }` to
  `&'static dyn RoleHarness` inside a closure that outlives its `Arc` — a latent soundness bug, not repaired here.
## 11. RESOLVED, later the same day (`lane/deep`): the collision is two names now

§3 said this was a rename, not a union, and recommended a rebase. The rebase then did what §3 warned about:
`tests/src/security.rs` on `main` became the L2 audit harness **with the L0 redirect policy glued into it** as an
associated function (`redirect_target`), so a redirect contract test calls a type whose other half owns a Postgres
pool. Two jobs, one name, and no author could tell which harness they were reading.

Captain's direction, 2026-10-08: keep both implementations, give each a name that states its job, export both, no
behaviour changes. What `tests/src/security.rs` holds now:

    RedirectPolicyHarness     L0 Pure        the production redirect policy (`safe_next`), no I/O
    AuditPersistenceHarness   L2 Persistence the production audit DAO on an isolated, disposable DEV target
    CommittedAuditRow         —              the L2 harness's read-back row (unchanged)

    tests/src/lib.rs  pub use security::{AuditPersistenceHarness, CommittedAuditRow, RedirectPolicyHarness};

Both are `pub use`d from `test_harness`, so `use test_harness::RedirectPolicyHarness;` and
`use test_harness::AuditPersistenceHarness;` are the two import lines, one per job. The redirect harness keeps its
body (the same `safe_next` call) and the audit harness keeps every method; nothing about what either does changed.

**How a blocked rebase resolves now — mechanically, five steps.** A branch that carried the redirect harness, or
the audit one, or both:

1. delete the branch's own `tests/src/security.rs` — trunk's file already holds the redirect policy under its own
   name, and duplicating it is the merge this page was written about;
2. take trunk's one-line export in `tests/src/lib.rs` verbatim (that is the "1-line export" conflict of §2);
3. `perl -pi -e 's/SecurityHarness/RedirectPolicyHarness/g'` over the branch's `sec_redirect__*` cases;
4. `web/src/api/google_auth.rs` — take trunk's (the CR/LF control-character fix);
5. a branch that carried the 363-line SEC.IDENTITY / SEC.ENTITLEMENT harness names it `IdentitySecurityHarness` in
   `tests/src/security_identity.rs` (§3). That name is now reserved for it and nothing on trunk holds it.

Step 5 is the reason the two trunk harnesses are named for their *jobs* rather than for the domain: the third
member of this family is an identity/entitlement harness, and a name like `SecurityHarness` cannot be split three
ways. The existing convention in this suite agrees — `RegistryHarness`, `SignatureHarness`, `ClientHarness`,
`EngineHarness` — a harness is named for what it lets a test do.

**Scope of the rename.** 32 code files, 34 with this page and the TST-REDIRECT hand-off: the module, the export
line, the 17 `sec_redirect__*` cases, the four `sec_audit__00[1-4]` cases, and the assertion-message `const HARNESS`
in the remaining security cases. That last group is worth a line: `sec_audit__005` and the eight
`sec_entitlement__*` L2 cases drive the production
`SecurityService`/`DurableSecurityAuditPort` over `TestDatabase` and never held the audit harness's type, so their
message now names the harness they actually use (`TestDatabase/L2 Persistence`) instead of borrowing a name that
belonged to a different harness.

## 12. The rename's receipts — and two red rows it did not create

Measured 2026-10-08 in `lane/deep`, `.env.local` sourced (`APP_ENV=development`, so the harness resolves
its DEV target; the harness refuses PRODUCTION before any socket opens).

    cargo check -p test-harness --all-targets                     EXIT=0, 0 errors
    cargo test -p test-harness --test <each of the 17 sec_redirect binaries>
                                                                  EXIT=0, 17 x "1 passed; 0 failed"
    cargo test -p test-harness sec_audit -- --ignored             EXIT=0, four L2 cases ok (001-004), DEV
    cargo test -p test-harness --test sec_audit__005__...         EXIT=0, 1 passed, DEV
    cargo test -p test-harness sec_entitlement                    EXIT=0, eight L2 cases ok (001-007, 009), DEV
    rustfmt --edition 2021 --check tests/src/{security,lib}.rs    EXIT=0

**Two SEC.REDIRECT cases were already red on trunk, and the rename only made them visible.** Both files come
from `b3ce7fef9` ("TST RED batch 45", authored with the web crate uncompilable, so nothing had run them), and
neither case's assertion is about the harness:

    sec_redirect__001__path_traversal:106    at 28ab66997 -> EXIT=101, left "/portal/dashboard",
                                             right "%2F%5Cevil.example"; after repair EXIT=0, 1 passed
    sec_redirect__004__percent_encoding:170  at 28ab66997 -> EXIT=101, `encode(" ")` (= "%20") failed a
                                             letters-only predicate; after repair EXIT=0, 1 passed

Both are fixture mistakes, not production defects: `encode` (`web/src/api/google_auth.rs:36`) is this site's
URI-component encoder and *should* encode `/` for the `client_id`, `redirect_uri`, scope and `back` values it
builds. Repaired in `7b293fb83`, with the receipts in that commit's message.

**rustfmt.** 18 of the 32 files this slice touched were already unformatted on `origin/main`, and T1's FMT
stage fails on any file a slice changed — it cannot tell dirt the slice did not create. Two numbers worth
keeping: the estate holds **201** drifted files and **199 of them are under `tests/`** — the SEC/CRM/workflow
batches wrote multi-line `assert_eq!`s and never ran rustfmt. `38d005b16` formats the 18 this slice touches
(formatting only; the 17 redirect binaries re-ran green after it). The other 181 are the quiet-trunk pass
§10.2 asks for, and they will block the next slice that touches one of them: a gate row with no owner.

**This slice's `pnpm slice:check`** (`.env.local` sourced, base = `c8b4f2aae`):

    T0 compile    PASS (cargo check --workspace --all-targets)
    FMT rustfmt   PASS (after 38d005b16; before it, 18 files this slice touched)
    T1 sections   FAIL — it stops at the first failing target, and that target is not one of this slice's:
                  arch_boundary__011__qa_cannot_own_git_mutations
                  tests/tests/arch_boundary__011__qa_cannot_own_git_mutations.rs:632
                  "forge/src/engine/assay.rs names `Command::new`"
    T2 full suite not run here (CI's tier)

**That red is a landed fix colliding with a guard, not this slice.** `Command::new` reached
`forge/src/engine/assay.rs` in `bd1b093f5` (FORGE-FIX-005, "bound every wait", landed on `main` via `530903627`),
and the QA sweep treats that file as QA surface (`arch_boundary__011:625-640`). It is on `origin/main` too. It is
an architecture decision — give the assay surface a bounded-wait door through the engine's command port, or take
the file out of the QA sweep — and §6's two other breaks in the same guard (lines 880 and 910) are still open.
Not repaired here: no slice may decide that, and it is not a test edit.

Because `slice:check` stops at the first failing target, the section was also run with `--no-fail-fast`
(`cargo test -p cli -p test-harness --no-fail-fast`, DEV sourced). Over the first **373** of the crate's ~600 test
binaries it found **two** red targets, both pre-existing and both about `forge/`, neither about a security file:

    arch_boundary__011__qa_cannot_own_git_mutations     forge/src/engine/assay.rs names `Command::new`
                                                        (bd1b093f5, above; :632, with §6's :880/:910 behind it)
    forge_arch_seam__001__canonical_execution_chain     the_job_layer_and_registry_know_no_role_no_node_and_no_vendor,
                                                        :281 — `forge/src/engine/job.rs` names `opencode`

The enumeration was stopped there deliberately: it is a survey, not this slice's gate, and it holds the lane's
target-dir lock while the push needs it. Whoever owns the queue should re-run it to the end (`cargo test -p cli -p
test-harness --no-fail-fast`) rather than trust these two numbers as complete.

## 13. CONVERGED (`lane/deep`, later the same day): one `SecurityHarness`, the two names deleted

§11's naming lasted one landing. Captain's direction, 2026-10-08 (`b85e73937`): there is one actual harness —
redirect policy through `web::api::google_auth`, identity resolution and entitlement decisions through the real
`web::security::SecurityService` (`decide` runs the production Casbin port), durable audit through the production
`DurableSecurityAuditPort`/`SecurityAuditDao` on the guarded DEV target — with `SecurityHarness::new()` as the
no-DB form and `connect_from_env`/`connect_declared` adding persistence. `RedirectPolicyHarness` and
`AuditPersistenceHarness` were left as *temporary aliases to that type* so trunk kept compiling while this lane
migrated the callers. This section records that migration: the aliases are gone.

**23 files, no behaviour change.** The 21 callers were renamed mechanically — the 17 `sec_redirect__*` cases
(`RedirectPolicyHarness` -> `SecurityHarness`, module docs included) and the four `sec_audit__00[1-4]` cases
(`AuditPersistenceHarness` -> `SecurityHarness`, their `connect_dev()` return type and the `const HARNESS` message a
failure prints included). An alias is transparent, so no assertion, fixture or call moved. One export line remains
(`tests/src/lib.rs:75`, `pub use security::{CommittedAuditRow, SecurityHarness};`) and the two `pub type` lines are
deleted; the names survive only as a tombstone in the module doc (`tests/src/security.rs:16-19`), so an older branch
can find out where they went.

**Receipts** (measured 2026-10-08 in `lane/deep`, `.env.local` sourced; `test-harness` reports one warning, unused
imports in `tests/src/crm.rs:24`, pre-existing and untouched — and with the aliases gone no `deprecated` warning is
left anywhere):

    cargo check -p test-harness --all-targets                       EXIT=0
    cargo test -p test-harness <each of the 36 sec_* binaries> -- --include-ignored
                                                                    EXIT=0, "36 passed; 0 failed; 0 ignored"
                                                                    (the four SEC.AUDIT L2 cases ran against DEV)
    rustfmt --edition 2021 --check <the 23 changed files>            EXIT=0

**FMT.** `tests/src/security.rs` arrived drifted in `b85e73937`: rustfmt reflows the multi-line `sqlx::query` in
`cleanup()` (`tests/src/security.rs:366`). This slice's FMT stage rewrote that one call — formatting only — and the
estate above ran after it. §10.2's 181-file quiet-trunk pass is still open and still unowned.

**T1.** This slice touches 23 files under `tests/`, so its section is `test-harness`; the estate above *is* that
section, run to the end over the 36 named binaries instead of stopping at the first failing target. §12's two
pre-existing red rows are in `cli`/`forge_arch_seam` targets and were not part of it. Nothing was narrowed to make
anything green: no baseline, allow-list or triage row was edited, and no SEC case was skipped (0 ignored).

**The identity/entitlement families stay where they are, and that is not a third harness.** They never held either
name. `sec_audit__005` and the eight `sec_entitlement__*` L2 cases drive the production `SecurityService`/
`ServiceRuntime` over `TestDatabase` + `SecurityDao`; the five `sec_role__*` L3 cases drive the production
`CasbinAuthorizationPort` directly, with no database at all. Folding the entitlement *mutation* cases onto the
canonical harness would not rename anything — it would change their subject: the harness's repository fixture is
read-only by design (`set_role_entitlement` answers `SECURITY_HARNESS_READ_ONLY`, "role mutation belongs in a
persistence contract", `tests/src/security.rs:427-436`), so a mutation contract driven through it would assert the
harness's refusal instead of the DAO's write. Nothing in this lane was a competing harness either: `tests/src/`
holds no `security_identity.rs`, and `grep -rn 'struct \w*Harness' tests/tests/*.rs` finds only the forge harnesses.

**§11's steps 3 and 5 are superseded; a blocked branch follows this instead.** The reserved third name
(`IdentitySecurityHarness`, `tests/src/security_identity.rs`) is *not* to be created: a branch carrying an
identity/entitlement harness folds it onto `SecurityHarness` (its `resolve_identity`, `decide`, `principal`, the
contexts, and — where it needs rows — `connect_from_env`/`rows_for`/`cleanup`) rather than landing a third type. A
branch carrying a redirect or audit harness renames its own name to `SecurityHarness`, not to one of §11's two.
`GuestHarness` stays unfolded deliberately: `mod guest` is private (`web/src/security/mod.rs:4`) and only
`GuestSignInService`, `guest_mail_from_env` and `GUEST_EMAIL_CODE_PROVIDER` are re-exported (`:8-10`), so
`GuestRepository` (`web/src/security/guest.rs:31`) is not a seam a contract test can name. A guest SEC test that
needs one is a story about exposing that seam, not a façade to fake.


## 14. `lane/muse`, attributed — which of the morning's landings are Forge and which are not (`lane/deep`, 2026-10-08)

The captain asked what Muse was actually doing, having told it to stop non-Forge work. Answered from git and not
from a status table: for each commit on `main` from `abe6513e5` up, the branch that delivered it
(`git branch -a --contains <sha>`), the same way §1 answered ancestry.

    through lane/muse:  abdc89512 DOCS-VAULT-FIXTURE: rebuild the 007 pdf-byte fixture against the current signer
                        abe6513e5 test(forge): repair the contract call sites the landed API changes moved
                        0a88d98f0 fix(sec-redirect): publish the origin/redirect_uri seam the SEC.REDIRECT tests import
                        23a950ca3 fix(sec-redirect): treat [::1] as loopback
                        9d12c571f style(sec-redirect): rustfmt the [::1] predicate
                        1bb900772 HANDOFF: section 10 — the three red targets repaired and landed
                        8f64efab4 HANDOFF: section 10.1 — the FMT and slice:check receipts
                        7b93c3e5a docs(handoff): the land queue is drained
                        3adb21fc4 fix(forms): draw Lisa's pre-signature from ONE resolver, issuance and preview alike
                        f07f8cea1 FIX-006-SOUNDNESS: own the harness behind Arc; delete the drive.rs:303 transmute
    through lane/deep:  c3e06974b … b3be12ec6 (nine commits, the `SecurityHarness` rename and its hand-offs)
    the merges:         530903627 (muse: FIX-004 24d032c9 + FIX-005 bd1b093f5) · 57927943a (muse: FIX-006-SOUNDNESS)
                        8a3b7f950 (fledge: FIX-008) · 1a0f60269 (nemotron-2: FIX-006) · d080c968d (nemotron: FIX-002/007)

**Three of those ten are the Forge work he was asked for** — FIX-004, FIX-005, FIX-006-SOUNDNESS. The other seven
are stories: two SEC.REDIRECT red targets from §10 (`0a88d98f0`, `23a950ca3`), the formatting of one of them
(`9d12c571f`), three hand-off sections for that same slice (`1bb900772`, `8f64efab4`, `7b93c3e5a`), and two that
belong to neither — the DOCS-VAULT 007 fixture (`abdc89512`) and the forms pre-signature resolver (`3adb21fc4`).
`abe6513e5` is Forge-adjacent (contract call sites the landed API changes moved) but it is not one of the ten fixes.
Every one of them arrived as an ordinary commit on `lane/muse`; no per-story worktree, no second workflow, and
nothing unattributable — `git worktree list` shows the standing lanes and no other tree.

**The stop costs two commits, and they are Forge commits.** `origin/lane/muse` is two ahead of `main`, tip
`9f0e07e0b FORGE-FIX-012: lift answer_lands/classify_change out of the Yew gate`. They are pushed, so they are
pullable; they are not on `main`, so by rule 6 they are unlanded rather than lost. Stopping the *non-Forge* work
therefore costs nothing that is on `main` and two commits that are Forge work — the two things are not the same
decision, and the branch can be landed as it stands.

**Both red guard rows are the price of the same morning, and neither author left its lane.** Row 1 of the
`TECH-DEBT.md` blocking table is FIX-005 (`bd1b093f5`, `lane/muse`) adding `Command::new` to
`forge/src/engine/assay.rs`; row 2 is FIX-006 (`32832c57c`, `lane/nemotron-2`) naming `opencode` in
`forge/src/engine/job.rs`. Nine fixes were merged between `47fce4b50` and `57927943a` and neither arch-guard target
was run against any of them: both are `test-harness` targets, the tiered gate a slice runs stops at the first red
target, and no step in the merge path ran them. That — not a rogue lane — is what put two red rows on trunk, and it
is why they were reported twice and assigned zero times: the report had no owner column.

**One label collision, so nobody assigns row 2 to the wrong lane.** Two different fixes are both called **FIX-006**:
`32832c57c` (lane/nemotron-2, "Fence lease on prolonged outage / stuck turns") is §9's FIX-006 and the one that put
`opencode` into `job.rs`; `f07f8cea1` (lane/muse, "FIX-006-SOUNDNESS", Arc migration, transmute deleted) is a
different commit with a different subject. Check the SHA, not the label.

**Next, in order.** (1) `pnpm slice:check` for anyone touching `tests/`, `forge/` or `cli/` will stop at
`arch_boundary__011__qa_cannot_own_git_mutations` until row 1 lands; (2) rows 3 and 4 are behind row 1 — row 3 needs
the captain's scope call, row 4 needs `lane/longcat`'s `3b32cc4ff` rebased and landed; (3) §10.2's quiet-trunk
rustfmt pass is no longer open: `6fe65a1a3` formatted the 182 files (181 under `tests/tests/`, one production file,
`forge/src/engine/job.rs`) with no token change outside rustfmt's own normalizations.

## 15. The same day, later: four of the six red rows closed (`1c55a567b`, lane/deep)

**The captain's three calls — "exclude tests", "land 008", "assign 8 9" — are landed on `main` as one commit,
`1c55a567b`.** Rows 3, 6, 8 and 9 of `TECH-DEBT.md`'s blocking table are closed; **three red targets remain**
(`arch_boundary__011` for rows 1 and 4, `forge_arch_seam__001` for row 2, `runtime_deploy__004` for row 10), and each
still names a lane and a date.

**Row 3 — the sweep reads the production tree.** `TEST_TREE = "tests/"` is one named, dated line beside `SELF`, and
the mutation-verb sweep now skips it. The file it was counting is `tests/tests/forge_assay__005__qa_cannot_modify_git.rs`,
whose subject *is* that QA may not push — it is the rule, not a site that could break it. Verified twice: by counting
(outside `tests/`, one file names a verb — `forge/src/engine/git_publish.rs`, once, which the guard itself asserts),
and by a run — with row 1's assertion neutralised for one run (temporary, reverted, `git status` clean; row 1 is
lane/muse's) the guard ran **past section 5** and stopped at `:924`, which is row 4. So section 5 is green on trunk and
row 4 is what stands behind row 1.

**Row 6 — the v5 money display.** Two display assertions took the two-decimal contract, and so did the display
property behind them: it was failing on `amount = "0000"` (0 commas, 1 expected) and had been masked by the two
assertions failing in front of it. Grouping is now measured on the *rendered* whole part, with exactly two decimals
required — stronger than what it replaced, and true.

**Rows 8 and 9 — the workflow call, made on evidence.** The engine is right and batch 60's two cases were authored
against rules no line implements. Completion is `count_active_tokens == 0`
(`middle/workflow/src/engine/execute_node_leave.rs:173-202`, both stores); an optional branch cannot prevent
completion because **the join retires it** — `handle_join` waits on required siblings only (`:42-44`), concludes each
still-active optional sibling `Completed`/`Skipped`, obsoletes its task and cancels its job (`:46-80`). That is what
`forge/definitions/RE_supermodel-v1.xml:199-204` says in its own words and what the same batch's two green cases
assert (`wf_join__002`, `wf_definition__011`). 006's original graph forked with **no join**, so nothing retired the
straggler and the engine correctly waited. 006 now has the join and asserts the retirement; 007's cancel half was
over-broad — it demanded `Cancelled` of *every* token, including the fork's own parent, which `handle_fork` had already
concluded `Completed` (`:385`). **No production line changed.** Both rows moved from lane/muse to lane/deep, since the
`middle/workflow` half needed no change.

**What that leaves.** Rows 1, 2, 4, 5, 7 and 10 are untouched by this pass, and rows 8 and 9 were the only two reds
that were CI-visible rather than environment-class.

**And the section gate says the same thing, in its own words.** `pnpm slice:check --since 98ef611dd` for this slice —
sections `harness`, crates `harness`, T0 PASS (52s), FMT PASS (2s), T1 **FAIL (219s)**, "DO NOT HAND OVER" — stopped
at `arch_boundary__011…rs:640`, row 1. That is not this slice's change failing: the four rows it closed were run by
name and are green. The useful part for whoever fixes row 1: **rows 2 and 10 are behind it too** — the section runs
`cargo test -p cli -p test-harness` without `--no-fail-fast`, and `arch_boundary__011` sorts before
`forge_arch_seam__001` and `runtime_deploy__004`, so neither can be observed in a section run until row 1 lands.
Row 1 is one line for whoever owns it: `forge/src/engine/assay.rs` needs its `Command::new` decision made (arch, not
a test edit), and row 4 is one rebase — `lane/longcat`'s `3b32cc4ff`.




## 16. The deploy WAS gated by the test suite; it is not any more (`lane/deep`, 2026-10-08)

The Captain asked whether the red rows could "blow up" a build, and then said what he wanted: *"i dont want tests to
block my deploy i dont know how that ever got in there … tests should be my choice to run not a gun to my head to do a
deploy."* He was right, and it was one script deep in the release path:

    $ grep -rlnE 'cargo (nextest|test)|pnpm test|slice:check' scripts/ .githooks/ vercel.json
    scripts/rust-dev-boot-smoke.sh
    scripts/build-all.sh
    scripts/ops/gate/slice-check.sh
    .githooks/pre-push
    $ grep -nE 'bash scripts/' scripts/release-record.sh
    135:  bash scripts/release-ci-check.sh "$CHECK_SHA"; CI_CHECK_RC="$?"

`pnpm release` → `scripts/release-record.sh:137-147` read `gates.yml` for HEAD and **exited 1** unless that workflow
completed `success`; `gates.yml:360-362` is `cargo nextest run --workspace --profile ci`, so a single authored-red
harness target refused every release. It arrived as `d5505faa2` (2026-09-18, "FORGE-LOCAL-RELEASE-CI-CHECK-01, Astra
feature 4") — a reviewer's hardening, not the Captain's rule — and had stood for twenty days.

**What changed (ruling, not preference): the read stays, the refusal moves behind a name.** `release-ci-check.sh` now
defaults to `RELEASE_CI_CHECK=read`: it prints the verdict for the exact sha — green, red, pending, no run, unreadable —
and exits 0 on every one of them. `require` is the opt-in gun (the old behaviour, byte for byte, behind a word);
`skip` reads nothing; an unknown value warns and is treated as `read`, so a typo cannot acquire a power the default
declines. One classifier serves both modes, so `read` and `require` can never disagree about what CI said — only about
what is done with it. Also fixed while in there: the verdict line was glued to the next log line (`$( )` strips the
trailing newline) and `workflow's success` in a node string inside a bash single-quoted block was a **syntax error** —
the file did not parse until that apostrophe left. `bash -n` clean, `shellcheck -S warning` clean.

Receipts — every mode, driven through the script's own offline reader (`RELEASE_CI_CMD`, which exists for this):

    ### read + CI RED (the captain ships anyway)        EXIT=0   "…did not succeed for deadbeef1234: run 8 attempt 1
                                                                (completed/failure) (informational: this does not stop
                                                                the release)"
    ### read + CI GREEN                                 EXIT=0   "green — gates.yml completed successfully…"
    ### read + NO RUN for this sha                      EXIT=0   "…has no run for deadbeef1234 (an unrelated workflow
                                                                success is not a substitute) (informational…)"
    ### read + UNREADABLE (reader exits non-zero)       EXIT=0   "UNREADABLE — could not read CI results…"
    ### read + PENDING                                   EXIT=0
    ### require + RED                                    EXIT=1   stderr: "REFUSED — the required workflow gates.yml did
                                                                not succeed…"
    ### require + UNREADABLE                             EXIT=1   stderr: "REFUSED — could not read CI results…"
    ### skip                                             EXIT=0   "MODE skip — CI will not be read at all…"
    ### typo (RELEASE_CI_CHECK=reqiure)                  EXIT=0   "WARNING — unknown … treating as read"

**What did NOT change, stated so nobody re-derives it:** the build and the sha-named live probe still decide what a
release row may claim; `pnpm deploy:prod` (`scripts/deploy-prod.sh`) never read CI or a test at all — it is Vercel
build + deploy; the pre-push hook runs only the `Cargo.lock` check and the wasm compile; `pnpm test:deploy-gate` is
`db:parity && test:app && test:forge:engine` and contains no harness. The ruling is written into `AGENTS.md` ("The gate
is tiered") and `docs/agent/MEMORY.md`, because the failure mode is an agent adding such a gate *as a feature*.

### 16.1 My triage of what is left after row 1 — put to the Captain, not acted on

Asked which rows I would fix, table or delete. My recommendation, in that order:

1. **Fix now, mechanical, no decision needed:** row 10 `runtime_deploy__004` (the test omits clearing the child's
   environment and sleeps a fixed 500 ms; the server's refusal is correct) and row 4 (`arch_boundary__011:924`, the
   assertion pins a rustfmt line-wrap spelling; `lane/longcat` `3b32cc4ff` already has the fix).
2. **Fix, but only after one sentence from him:** rows 1 and 2 — the guard caught a real boundary change (a process
   spawn in the QA engine module, `assay.rs`, so a wait can be bounded; the job layer reaching `opencode::turn_ceiling`),
   and the question is whether the rule widens to "no git door, any process" or the code moves. Both guards predate the
   code, so the guard did its job; neither is a user-visible fault.
3. **Table:** row 5 and the duplicate-id estate (17 SEC.REDIRECT files for 11 ids) — real, but nobody is blocked.
4. **Delete:** nothing, yet. The brittle assertion (row 4) should be *rewritten*, not deleted: a guard deleted for being
   inconvenient is how a written rule stops being enforced, and this repo has already paid for that once. If the
   Captain wants `slice:check` unblocked before rows 1/2 are ruled on, the honest move is a **named, dated quarantine
   list the gate reads** — never a green-washed baseline.

