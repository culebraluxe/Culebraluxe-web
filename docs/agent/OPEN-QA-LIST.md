# OPEN QA LIST

Things we have agreed to circle back on. The captain asked for this list on 2026-09-28, after the Apple
Messages port landed: work that is done but not yet proven in production, work that is deliberately paused
for him, and loose ends found while porting.

Rules: every row has a name, a place and an exit — the exact check that closes it. Ordered oldest-blocking
first. Nothing here is a wish list; if it is not worth an exit it does not belong.

Last reviewed: 2026-10-14 (`Culebraluxe-web`, `61b097dcd`) — **the coverage debt is closed**: every id in rows 10–17 is cited by a contract test on `main`, and the check that says so is in the section below. Previous review 2026-10-13 (`6a624335e`) — the nine unlanded TST-RED batches measured and recorded below (rows 10–17). Previous review 2026-09-28 (Apple Messages intake port, `5df24a09`, `1e6c077f`, `8d4f2b7e`).

## Paused by the captain — do not touch until he says go

| # | Thing | Why it waits | Exit that closes it |
| --- | --- | --- | --- |
| 1 | **First PROD run of the Rust Apple Messages intake.** | He is linking clients by hand; a PROD intake run would collide with that work. | He says go → `pnpm apple:sync:run` (or the 08:00/18:00 tick) → tally read back: inserted/replayed/evidence rows, then a second run that lands nothing new. |
| 2 | **Re-arm the launchd job** `com.culebraluxe.apple-sync` with the new script. | The job's deployed copy (`~/Library/Application Support/CulebraLuxe/apple-sync.sh`, Aug 27) still calls the dead TypeScript intake, so it fails with exit 1 and **writes nothing to PROD**. That is what keeps him safe right now, and it keeps being true until `pnpm apple:sync:install` is run. | After item 1 passes: `pnpm apple:sync:install`, then one tick at 08:00 or 18:00 reported SUCCESS with the intake tally in `~/Library/Logs/CulebraLuxe/`. |
| 3 | **`apple:repair:prod` on a real database.** Now Rust (`apple-sync messages-intake --evidence-only --refresh`); proven on DEV only. | Same collision as item 1 — it writes evidence to PROD. | One run when he is clear of the linking work, then the Client read models read back with the repaired channel rows. |

## Found while porting — needs a real look

| # | Thing | Why it matters | Exit that closes it |
| --- | --- | --- | --- |
| 4 | **Apple timestamp field names.** The exporter writes `dateISO`; serde read `dateIso`, so **every message silently lost its timestamp**. He says this class of bug "is always biting us in the ass". Fixed for `dateISO` (pinned by a test) — the rest is audited. | A message without a timestamp is dropped by the rules, not refused. A whole sync could report success while landing nothing, which is exactly the failure the intake exists to end. | Audit every field the Swift exporter emits against the Rust structs that read it (`apple_messages.rs`, and the Mail/Gmail exporters when they are ported); make a missing timestamp a **refusal or a counted skip**, never a silent drop; one test per field. |
| 5 | **Remaining Apple chain still dead or missing.** Calls intake, contacts load, contacts project, warehouse promotion (no Rust home at all), Apple Mail envelope, Apple Mail promotion. `middle/apis/src/apple/mod.rs` is still a 4-line placeholder. | The twice-daily job is one of eight files' worth of behaviour; the intake half is the only half fixed. | Each ported to Rust with a `scripts/rust-live-check/` check on DEV, and its `pnpm`/`.sh` caller pointed at the Rust command. |
| 6 | **WhatsApp.** He said "we fix apple, whatsapp stuff". The live path is already Rust (`db/src/whatsapp.rs` — landing, evidence, interaction, read models); it was not touched. | Not yet clear what he saw failing. Guessing would repeat the mistake of fixing the wrong thing. | He names the failing behaviour → reproduce it → fix, with the coexistence tests left alone as instructed. |

## Cleanup — decided, not yet executed

| # | Thing | Why it matters | Exit that closes it |
| --- | --- | --- | --- |
| 7 | **The dead `package.json` menu: 64 entries pointing at files that no longer exist** (of 145). Includes `forge:packet-lint`, `forge:sync-agents`, `forge:harness`. | Commands a future script or agent may still call. He did not know they were there. | Each entry either removed or re-pointed at a Rust command; `pnpm broken:ts:sweep` and a `grep` for the removed names both come back clean. |
| 8 | **`scripts/vercel-build-prod.sh` calls the dead harness** (`pnpm forge:harness`, lines 68–70) as reported-not-fatal. | The production build runs a gate that cannot even load, and prints whatever it manages to say. A dead step in the release path is a step that can only fail or lie. | Remove the call, or make it a real Rust check. Decide with item 9. |
| 9 | **`forge-packet-lint` ("lint") and `forge-sync-agents` — CLOSED 2026-09-27: captain says keep, and they are Rust now.** | Lint checks the agent paperwork, never the app; it was stamped `⚠ BROKEN ON PURPOSE` and imported five deleted modules, so `pnpm forge:packet-lint` could not run at all. Sync-agents copies four rules from `AGENTS.md` into the vendor pointer files. | **Done**: `cli` `forge harness-lint` + `forge sync-agents` (one rule source, `forge::vendor_block`), 39 fixtures, `pnpm` entries re-pointed. First live run: 0 failures, 175 baselined warnings, and it found the corrupt `docs/agent/harness-lint-baseline.json`. Still open inside this item: `ORIENTATION.md`, `MAP-engine.md`, `MAP-services.md` cite the deleted TypeScript tree and must be re-pointed at `rust/`. |

## Coverage debt — the nine TST-RED batches SPACE BUNNY never landed (2026-10-13, closed 2026-10-14)

**CLOSED 2026-10-14 (`61b097dcd`) — all 75 ids below are cited on `main`.** The exit was run as one check over
every id in rows 10–17 — `grep -rl "TST-<ID>" tests/tests/*.rs` returns the id's own file — and the files that
close these rows are `sec_identity__001..009`, `sec_entitlement__010`, `forge_scope__001..009`,
`forge_release__005/006/008/009/010/011`, `int_mux__002/003`, `int_whatsapp__001..006`, `obs_error__001/002`,
`property_media__008..010`, `property_public__001..007`, `ui_accessibility__001..005`, `ui_entitlement__001..005`,
`wf_definition__001..004/006..010`, `wf_decision__010`, `wf_timer__003..010`, `wf_token__001/003` (75 files, one per
id). The coverage was **re-authored against the refactored rails**, not landed as written, and the rot was diagnosed
per test rather than papered over: the batch's worst case, `chaos_forge__005`, found its recovery function **dropped
by migration 279**, its ledger insert fenced by the generation-binding trigger (so the claim is a precondition, not a
formality), and its recovery straddling **two** staleness clocks (the execution heartbeat and the work item's
`updated_at`). Rows 10–17 are kept exactly as written, each marked with its closure, because the scope recorded below
is the spec the tests were written to.

The captain's rule for these: discard them as code if the suite already covers their scope, otherwise record the
scope so the tests get written. Measured on `6a624335e` the way this repo names coverage — every contract test
names its story (`//! SEC.AUDIT — denied action logged (TST-SEC-AUDIT-001)`) — **75 of their 82 story contracts
were cited by no test on `main`** until 2026-10-14. Seven are not debt: batch 22's five files are on `main` in `main`'s own
authoring (`forge_qa__013`, `forge_release__001..004`), and batch 7's two `db_schema__003/005` contracts are on
`main` in smaller form. So the code is dropped, the scope is recorded below, and the batches stay preserved at
`refs/wip/lane-spacebunny-tip` (`399418c0b`), `refs/wip/lane-spacebunny-held-sec` (`9bce7af49`),
`refs/wip/batch44-f5d38ed5` (`f5d38ed54`) and `origin/lane/spacebunny`.

None of the nine is landable as authored. Batch 44 was taken into a scratch worktree on `6a624335e` and **does not
compile**: `error[E0432]` on `test_harness::security::{edge_context, infrastructure, GuestHarness}`, which the
harness refactor (`df7d736fc`, `b3be12ec6`) replaced with methods on `SecurityHarness`; `test_harness::source` is
gone, and `web/src/security/mod.rs` never exported `GuestRepository` (the batch's one-line carrier for it). All
nine share that shape — authored against the Oct-7 tree — which is why re-authoring the coverage as the code
arrived was cheaper than landing them, and why they are here instead.

Every exit below is one check: the named story has a contract test on `main` that cites it. The preserved batch is
the spec — read it before re-inventing the assertion.

| # | Thing | Why it matters | Exit that closes it |
| --- | --- | --- | --- |
| 10 | **CLOSED 2026-10-14 (`61b097dcd`) — SEC.IDENTITY 001–009 and SEC.ENTITLEMENT-010** — batch 44 (`9bce7af49`, also `f5d38ed54`): the `(provider, subject)` pair as the key, `Unmapped` as a non-principal, account linking, machine login, immutability, external/internal guest, break-glass, disabled identities, session expiry, guest document. | `sec_identity__*` is **zero files on `main`** — the identity boundary a forged `sub` arrives at has no contract test, though the service is live (`resolve_identity`, `decide` in `web/src/security/mod.rs`; `db/src/security.rs`). | Ten tests citing `TST-SEC-IDENTITY-001..009` and `TST-SEC-ENTITLEMENT-010`, written against today's `SecurityHarness` (real policy, faked repositories); the `GuestRepository` export comes first. |
| 11 | **CLOSED 2026-10-14 (`61b097dcd`) — FORGE.SCOPE 001–009 and FORGE.RELEASE-011** — batch 23 (`ee359a8a9`): candidate descends from the recorded base, only a lane's own commits count, foreign concurrent commits excluded, scope additions, renames, deletions, root paths, base never moving, smoke-failure classification. | `forge_scope__*` is **zero files on `main`** while `forge/src/engine/scope.rs` and `forge/src/scope_manifest.rs` are live — these decide whether a lane's result is admissible at all. | Nine tests citing `TST-FORGE-SCOPE-001..009`, one citing `TST-FORGE-RELEASE-011`. |
| 12 | **CLOSED 2026-10-14 (`61b097dcd`) — INT.MUX-002/003, INT.WHATSAPP-001..006 and OBS.ERROR-001/002** — batch 30 (`bba50486a`): failure mapping, identifier preservation, webhook verification, payload normalisation, attribution, duplicate delivery, message state, malformed input, and the two error-classification rows. | `int_*` on `main` is ten files, **all `int_apple__*`**; `obs_*` is three, **all `obs_flight_recorder__*`**. WhatsApp is live (`db/src/whatsapp.rs`, `middle/apis/src/whatsapp/*`) with no failure-side contract. | Ten tests citing those story ids. |
| 13 | **CLOSED 2026-10-14 (`61b097dcd`) — PROPERTY.MEDIA-008..010 and PROPERTY.PUBLIC-001..007** — batch 39 (`4155dcf22`): MIME validation, video upload, media ownership, publish eligibility, slug, public read, unpublished case. | The best-covered family on `main` (`prop_*` 12 files, `property_land__*` among 24) is missing exactly the two corners where a listing leaves the building: publishing and media (`db/src/media/*`, `db/migrations/082_property_publication.sql`). | Ten tests citing `TST-PROPERTY-MEDIA-008..010` and `TST-PROPERTY-PUBLIC-001..007`. |
| 14 | **CLOSED 2026-10-14 (`61b097dcd`) — FORGE.RELEASE-005, 006, 008, 009, 010** — batch 22's remaining five (`dbec1c84d`): smoke verification, batch deployment, release receipts, publish gate, deployment record. | Its first five are already on `main`, so the release rail (`forge/src/engine/release.rs`) is proven to 004 and unproven from 005 — a gap in the middle of the rail the acceptance record rides on. | Five tests citing those ids. |
| 15 | **CLOSED 2026-10-14 (`61b097dcd`) — UI.ACCESSIBILITY-001..005 and UI.ENTITLEMENT-001..005** — batch 51 (`ab8e88901`): selected/current states, labels, button names, table headers, form errors; hidden, disabled, read-only and root-screen entitlement. | Both prefixes are **zero files on `main`**, while the UI already carries the markers (`aria-label` in `web/ui/src/app/chrome.rs` and the clients/forms/projects/flight-recorder screens) and the entitlement policy is real. | Ten tests citing those ids. |
| 16 | **CLOSED 2026-10-14 (`61b097dcd`) — WF.DEFINITION-001..004, 006..010 and WF.DECISION-010** — batch 55 (`cd96eae97`): parser, validation, version identity, duplicates, invalid targets/edges, unsupported nodes, cycles; fact refresh. | `wf_definition` on `main` is `__005` and `__011..013` — the family exists but the parser and validator rows do not, from a live engine (`middle/workflow/src/engine/*`). This is the one row whose production half was not located by name: the exit starts by finding the parser the story means. **Found 2026-10-14: it is `parse_process_definition_xml` / `definition_from_xml` (`forge/src/engine/xml.rs:352-412`, `:468-480`) — the parser `deploy_xml` (`forge/src/engine/deploy.rs:83-92`), the engine binary (`forge/src/bin/forge_task.rs:45`) and `validate_definition_xml` (`forge/src/engine/validate.rs:16-75`) all call.** | Ten tests citing those ids. |
| 17 | **CLOSED 2026-10-14 (`61b097dcd`) — WF.TIMER-003..010 and WF.TOKEN-001, 003** — batch 59 (`399418c0b`): lease prevents duplicate firing, stale lease reclaim, retry backoff, reschedule, cancel, timer against moved state, multiple works, max attempts; token creation, stale token. | `wf_timer__*` on `main` is two files (`__001`, `__002`) against eight unproven rows, and `wf_token__*` has `__002/004/005/006` but not `001/003`. Timers and tokens are where a workflow can act twice (`middle/workflow/src/engine/fire_timer_job.rs`, `execute_node_leave.rs`). | Ten tests citing those ids. |

