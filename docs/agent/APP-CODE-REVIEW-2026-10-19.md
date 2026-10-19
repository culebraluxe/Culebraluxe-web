# App code review — Culebraluxe-web (2026-10-19)

Purpose: establish the app baseline and the real Forge boundary **before** any `re-host`/web change.
Read-only review + baseline gates; no app code changed.

## 1. Baseline (measured)

| gate | result |
|---|---|
| `cargo check -p web -p db -p cli --all-targets` | **Finished** |
| `cargo test -p web --lib` | **162 passed, 0 failed** |
| `cargo test -p db --lib` | **65 passed, 0 failed, 4 ignored** |
| `TODO/FIXME/unimplemented` in web/middle/db/cli | **0** |
| `unwrap/expect/panic` occurrences (web/src) | **303** (needs triage; includes tests) |
| `unwrap/expect/panic` occurrences (db/src) | **43** |

Size: web 279 Rust files / ~45,281 lines; db 102; cli 48; middle/model 62, services 15, workflow 3,
apis 13; test-harness 741. Web has 122 route declarations. Largest web modules: `luxesign/mod.rs`
2,785, `command_runtime.rs` 1,765, `signer/mod.rs` 1,431.

## 2. Existing partition (what the principal tried to keep)

- **Forge code sits in its own directory** (`forge/`) and is a distinct crate with its own tests/bins.
- But it is **not isolated at the build boundary**: `web` depends on `forge` and mounts `ForgeService`
  in-process; `db` and `cli` are shared; one migration chain; one test-harness.
- App → Forge surface is **smaller than feared**: 6 real web files (`composition.rs`,
  `http_runtime.rs`, `api/engine.rs`, `agreement_execution.rs`, `api/tech_page.rs`,
  `api/portal_bridge/tech_documents.rs`); `ui_auth.rs`/`signer_edge.rs` "forge" hits are the word
  "forged", not Forge.
- DB → Forge: 116 Forge mentions across 19 db files: the `forge_*` modules (proper Forge DAOs living
  in the shared `db` crate), plus app files that reach Forge tables (`tech.rs` 555 lines / 34 refs,
  `boot_gate.rs` forge migration list, `pool.rs`/`schema_parity.rs`/`person.rs`/`error.rs`/
  `flight_recorder.rs` incidental).
- CLI → Forge: 34 files (30 under `cli/src/forge/`, plus `main.rs` dispatch, `launchd/agent_worker.rs`,
  `db_tool.rs`).
- Tests → Forge: ~309 test-harness files reference Forge (of 741). Docs: 216 Forge-related files;
  scripts: 18.
- Docs orientation risk: `docs/agent/CURRENT.md` is dated 2026-10-02 and calls itself the fresh-agent
  entry point; the live machine is newer. Do not trust its path inventory without checking code.

## 3. App-specific findings/risks

| # | Finding | Why it matters |
|---|---|---|
| A1 | App server hosts the workflow engine and Forge service in-process (`web/src/api/engine.rs`, `composition.rs`, `http_runtime.rs` warm-up). | This is the extraction seam; an HTTP boundary is required before removing `forge` from `web`. |
| A2 | The app writes Forge tables directly: `db/src/tech.rs` (34 refs) and cockpit/tech-document paths. | App ownership of Forge tables must become a read API before schema/code ownership moves. |
| A3 | `db` crate owns both app and Forge DAOs plus boot gate/migration ledger; chain is interleaved (44% of migrations touch Forge tables). | No DB split; one ledger/applier must survive the code move. `jobs` exists in DEV/PROD but is absent from the replay chain (migration-record gap). |
| A4 | `cli` binary mixes app commands, Forge commands, and a launchd worker. | CLI split is part of X-4; until then extraction cannot claim a clean boundary. |
| A5 | ~309 test-harness files reference Forge; CI/gate mixes both systems. | Moving Forge code strands tests; the cut must include a test-ownership plan. |
| A6 | 303 `unwrap/expect/panic` sites in web/src (many tests, but not triaged). | Not an extraction blocker, but any production-readiness claim for the app needs a panic audit. |
| A7 | Large cohesive modules (`luxesign` 2,785; `command_runtime` 1,765; `signer` 1,431). | Refactor candidates; do **not** mix them into Forge extraction (behavior risk). |
| A8 | App is otherwise clean of TODO/FIXME markers; web/db test suites green. | Good baseline; extraction should preserve it with tests at every step. |

## 4. Verdict on partition

The principal's instinct is right: Forge is **directory-isolated but build-coupled**. The app-facing
Forge surface is only ~6 files, so the extraction is tractable; the hard coupling is the shared
`db` crate/migrations, shared `cli`, and test-harness, not app business logic.

**Do not refactor app internals as part of the extraction.** Treat the app as a frozen consumer:
replace `forge::`/engine calls with `re-host`/`forge-api` behind tests; turn direct Forge-table reads
into API reads; leave luxesign/signer/command_runtime untouched.

## 5. App review sign-off checklist (before X-2 touches anything)

- [x] app workspace check
- [x] web lib tests (162)
- [x] db lib tests (65)
- [x] Forge-coupling inventory (6 web files, 19 db files, 34 cli files, 309 tests)
- [x] partition verdict
- [ ] principal approval of the X-2 boundary plan (next)
- [ ] re-run baseline immediately before the first web edit and after every step
