# Handoff — the Forge operator surface port (2026-09-28)

The Forge pieces that never crossed the line: most of the operator surface was dead because every `forge:*`
command ran a TypeScript file importing `legacy/db/*`, deleted in `4cf98110`. This session ported the read tools,
the doctor, and the writer. What is left is below with the exact files to open.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | `forge board`, `forge story-show`, `forge batch-status`, `forge doctor`, `forge reset/recover/clean` all answer from the real control plane | `APP_ENV=dev cargo run --manifest-path rust/Cargo.toml -p cli -- forge board` (DEV, read-only) |
| S2 | The reset tool refuses DEV and any non-`--force` PROD invocation | `APP_ENV=dev … forge clean --force` → "refusing to run against DEV…" |
| S3 | Rust workspace is whole: `cargo check --workspace --all-targets` exits 0; `cargo test -p cli` 61, `-p forge` 58, `-p db` 38 pass | `cd rust && cargo test -p cli -p forge -p db` |
| S4 | `pnpm forge:manifest`, `forge:triage`, `forge:board-sync`, `pnpm sprint`, `pnpm health`, `pnpm test:story` and ~14 more still point at `⚠ BROKEN ON PURPOSE` TypeScript | sweep `package.json` for `scripts/forge-*.ts` with the banner; priority list in `docs/agent/BROKEN-TS-INVENTORY.md` §2 |
| S5 | `pnpm test:harness` is red for content reasons, not only dead files: `protected-files.test.ts` (1 fail), `test-sections.test.ts` (4 fails), `rust-parity-ledger.test.ts` (3 fails) | `node --import tsx --test scripts/test-sections.test.ts` |
| S6 | `.github/workflows/gates.yml` still runs `scripts/forge-static-gate.ts`, which imports `@/legacy/workflow_app/forge/forge-static-gate` — that path does not exist, so the step cannot start | `ls legacy/workflow_app/` → README.md, definitions, tests (no `forge/`) |
| S7 | `cargo fmt --all -- --check` carries 538 pre-existing diffs (ui 184, server 291, cli 50) | `cargo fmt --all -- --check \| grep -c '^Diff'` |
| S8 | `AGENTS.md` cites guards under `workflow_app/tests/`, which no longer exists | `ls workflow_app` → no such directory |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | Any PROD-mutating Forge run — including `pnpm forge:clean`, which sets `APP_ENV=production` and passes `--force` | the Captain | ask first, every time; DEV is free |
| H2 | `rust/ui` and the calendar/projects screens | the Captain, mid-rebuild with Claude | do not reformat, do not "improve", do not touch |
| H3 | `AbstractService` as the design rule; calendar's hand-written impl is correct | the Captain (ruling) | never replace it with `abstract_service!` |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Port another `forge:*` command | `docs/agent/BROKEN-TS-INVENTORY.md` §2 | `rust/cli/src/forge/mod.rs` (dispatch) + a module beside `read_tools.rs` / `doctor.rs` / `reset.rs` |
| Add a read | `rust/core/db/src/forge_read.rs` (views + normalisation at the boundary) | that file, then `rust/cli/src/forge/read_tools.rs` |
| Add a write | `rust/core/db/src/forge_reset.rs` (one writer, guards) | that file + `rust/cli/src/forge/reset.rs` for the CLI guards |
| Make the CI gates honest | `.github/workflows/gates.yml` lines 87–100 | `package.json` (`test:harness`), `scripts/test-sections.ts`, `scripts/protected-files.ts` |
| Know where SQL may live | `AGENTS.md` "Rust First" | SQL only in `rust/core/db/**`; the CLI holds no query |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `279e0aea` | read tools in Rust: `forge board` / `story-show` / `batch-status`, `ForgeReadDao` over the five views of migration 191 | `cargo test -p cli` 43, `-p db --lib` 38; `APP_ENV=dev … forge board` → 2 batches; `story-show ENG-FORGE-DOCTOR-01` → receipt + 1 hold + 57 findings |
| `2823085b` | `forge doctor` in Rust: pure renderer, QA agreement, ROI rollup, `ForgeDoctorDao` — and one bug the DEV run exposed (the oldest-claim read selected STALE claims instead of excluding them) | `cargo test -p cli` 51, `-p forge` 58; `APP_ENV=dev … forge doctor` → BUSY, 79 instances, 2 open work items, WORKER FAILING, QA 23 checked / 1 DISAGREE named |
| `7f3ed6ed` | `forge reset/recover/clean` writer + the guards as a pure tested function; stale claims go through the engine's own recovery path, `RETURNING` row count kept load-bearing | `cargo test -p cli` 61; `APP_ENV=dev … forge clean --force` → refuses DEV |
| `f9734772` | (earlier session) the load-dependent harness-lint fixture flake | `cargo test -p cli` 39 |
| `02145a8b` | (earlier session) the deploy doc rewritten to the real one-container flow | docs reviewed against `deploy/Dockerfile.build` + `scripts/deploy-prod.sh` |

## 5. NOT VERIFIED — the honest gaps

- **No PROD run of any new command.** Every fact above is DEV (reads) or a refusal (the writer). The writer's
  happy path is unit-tested only through its guard function; its SQL steps have not executed anywhere, on purpose.
- `forge doctor`'s ROI section printed "(no ROI rows in the window)" on DEV. The rollup is unit-tested against
  fixtures, but no live row has passed through it.
- The 538 fmt diffs: not run, not fixed, left alone on purpose (H2).
- `pnpm test:harness`, `pnpm lint` and the GitHub `gates.yml` steps are red and were NOT fixed this session.
- The WORKER: FAILING reading came from the owner's real invocation log on DEV. Nothing was done about it; the
  worker is not part of this story.

## 6. OPEN — the next actions, in order

1. **`forge:manifest`** (`scripts/forge-manifest.ts`, 532 lines, dead). Port to `rust/cli/src/forge/manifest.rs`
   plus a read in `rust/core/db`. Finished when `pnpm forge:manifest <STORY-ID>` writes
   `docs/agent/manifest/<STORY-ID>.md` and `--check-all` exits 0. This also unblocks the `pnpm forge:harness`
   chain: `forge:sync-agents` ✓ → `forge:manifest` ✗ → `forge:packet-lint` ✓ → `test:harness` ✗.
2. **The harness gates** (S5, S6), cheapest first:
   - a. `docs/agent/ORIENTATION.md` line 1 uses a hyphen where `scripts/protected-files.ts` protects an em dash.
     Restore the em dash and `protected-files.test.ts` goes green.
   - b. `scripts/test-sections.ts` classifies the DELETED TypeScript tree (`legacy/workflow_app/tests`,
     `testv2/engine_tests`, `agent-runtime`). Rewrite `SECTION_RULES`/`listTestFiles` for today's tree (`rust/**`
     tests plus the surviving `scripts/*.test.ts`), then update `scripts/test-sections.test.ts`'s expectations.
     Do not delete the sections: an empty section is the lie that test exists to catch.
   - c. `scripts/rust-parity-ledger.ts --check` fails (79→111 routes of drift, plus unclaimed routes). The
     generator rewrites `docs/rust-parity-ledger.md`; decide whether the missing claims belong in the ledger or
     the routes are genuinely unclaimed, then commit the regenerated file WITH that fix.
   - d. `package.json` `test:harness` is a glob over six dead suites. Point it at the four that pass (the Rust
     tests already cover the three forge ones) instead of at a glob that grows again.
   - e. `.github/workflows/gates.yml`: replace the `forge-static-gate` step with the checks that exist now (the
     Rust persistence-boundary grep already lives in `rust-client-cutover.yml`). It is decorated, not enforced.
3. **The remaining dead `forge:*` commands**: `forge:triage`, `forge:record-stories`, `forge:test-stories`,
   `forge:ladder`, `forge:story:run`, `forge:scorecard`, `forge:sync-history`, `forge:board-sync`, `forge:tools`,
   `forge:decision`, `forge:learn`, `forge:roi`, `story:status`, `story:preflight`, `sprint`, `health`,
   `test:story`, `test:sprint-fences`. Port in that order: the first five are the engine's own dispatch surface,
   the rest are reports.
4. **`AGENTS.md` guard paths** (S8): repoint the `workflow_app/tests/*` citations at the Rust tests that now own
   those rules, or say where each is enforced instead. `pnpm forge:packet-lint` reports them as warnings until then.

## 7. ASK THE OWNER

- **Which open block comes first — the manifest (1) or the harness gates (2)?** On `manifest` the next agent opens
  `scripts/forge-manifest.ts`; on `gates` it opens `docs/agent/ORIENTATION.md` and `scripts/test-sections.ts`.
- **May `pnpm forge:clean` be exercised once against PROD, with the Captain watching, so the writer's happy path
  is verified rather than assumed?** Yes → the printed post-condition is the receipt. No → the writer's SQL stays
  unverified, which is recorded above rather than discovered later.
