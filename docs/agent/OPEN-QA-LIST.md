# OPEN QA LIST

Things we have agreed to circle back on. The captain asked for this list on 2026-09-28, after the Apple
Messages port landed: work that is done but not yet proven in production, work that is deliberately paused
for him, and loose ends found while porting.

Rules: every row has a name, a place and an exit — the exact check that closes it. Ordered oldest-blocking
first. Nothing here is a wish list; if it is not worth an exit it does not belong.

Last reviewed: 2026-09-28 (Apple Messages intake port, commits `5df24a09`, `1e6c077f`, `8d4f2b7e`).

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
