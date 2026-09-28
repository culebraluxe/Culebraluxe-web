# Dead commands — the `pnpm` menu, keep-or-delete

Measured **2026-09-28** by `node scripts/dead-command-sweep.mjs` (`pnpm broken:ts:commands --check`).

`docs/agent/BROKEN-TS-INVENTORY.md` is the file-by-file ledger: which FILES cannot load, and what each
capability's Rust home is. This page asks the same question from the operator's side: **which COMMANDS
cannot run, and what is the decision on each one.** A command that names a banner file fails with
`ERR_MODULE_NOT_FOUND` before it does anything, so the menu advertises work that cannot happen — and
three of them are named in runbooks as if they worked.

## The honest number

145 `package.json` scripts. **53 name a banner file.** Zero name a file that is missing from disk: every
reference resolves to a file that exists and is marked dead.

| Block | Commands | What it is |
| --- | --- | --- |
| Forge / story operator surface | 17 | the Forge harness and the board feeders: one Rust `forge` subcommand each |
| agent runtime | 3 | the retired Node agent loop (`rust/forge/src/bin/forge_worker.rs` is its replacement) |
| retired server stack | 33 | the old TypeScript services, their probes (`verify:*`), loaders and sync jobs |

Three corrections, recorded so the number stays honest:

1. The earlier figure of 56 (21 + 3 + 32) was a hand count. The sweep says **17 + 3 + 33 = 53**, and the
   sweep is the authority: it reads `package.json` and the banner itself, nothing else.
2. `check:widgets` → `scripts/check-svar-widgets.mts` was absent from every earlier count because the
   pattern did not include `.mts`. It does now, so it cannot hide again.
3. `forge:roi` was ported on 2026-09-28 (`be6df89d`) and has left this list.

The count may only fall. `--check` fails when it rises **and** when it falls without the baseline being
lowered, so this page and the tree cannot drift apart silently.

## Block A — Forge / story operator surface (17)

The rules belong in `rust/forge/src/`, the reads and writes in `rust/core/db/src/`, the command in
`rust/cli/src/forge/`, and `package.json` is repointed in the same commit. That is how `forge:roi` landed
and it is the shape every row below is meant to take.

### Done

1. `forge:roi` (was `scripts/forge-roi.ts`) → `rust/cli/src/forge/roi.rs`, rules in `forge::roi`
   (`parse_window_days`, `render_roi_report`), read in `ForgeDoctorDao::roi_attempts`. `be6df89d`.

### Next, in this order

| # | command | script | verdict | Rust home, and what is missing |
| --- | --- | --- | --- | --- |
| 2 | `forge:triage` | `forge-triage.ts` (84) | PENDING (captain classifies) | rules → `rust/forge/src/`, TypeSafe call → `rust/integrations/`, command → `rust/cli/src/forge/triage.rs`. **Nothing to copy from: `legacy/db/forge-typesafe-triage.ts` is deleted**, so the reference is `docs/agent/typesafe-failure-triage.md` plus the script's own help text. **No `triage` table exists in `db/migrations`** — where an observation lands has to be decided before this is written |
| 3 | `forge:test-stories` | `forge-test-stories.ts` (3498) | PORT P1 | the biggest of the feeders; one `forge` subcommand, DEV/PROD explicit |
| 4 | `forge:record-stories` | `forge-record-stories.ts` (234) | PORT P1 | same subcommand family as 3 |
| 5 | `forge:ladder` | `forge-ladder.ts` (151) | PORT P2 (lowest of the feeders) | same |
| 6 | `forge:story:run` | `exec-batch1-story.ts` (79) | PENDING (captain classifies) | the engine already dispatches; the live manual equivalents are `forge:clean` and `forge:story:reset`. Decide: repoint to a `forge` subcommand, or RETIRE the name |
| 7 | `forge:decision` | `forge-decision.ts` (318) | PENDING | half is done: `rust/cli/src/forge/decision.rs` (shape rules, used by the lint). Missing: `forge decision check` — mirror versus row, over `db::forge_engine::active_decisions` |
| 8 | `forge:learn` | `forge-learn.ts` (88) | VERIFY | the loop it calls (`agent-runtime/learn-loop.ts`) is dead, so the Rust home is the engine's own learn pass in `rust/forge/src/engine/`. `forge doctor` already reads the pass anchor (`.forge-context/learn-last-run.json`) |
| 9 | `forge:scorecard` | `forge-scorecard.ts` (83) | PENDING | rules → `rust/forge/src/scorecard.rs` (pure), read → `db::forge_read`/`forge_doctor`, command → `rust/cli/src/forge/` |
| 10 | `forge:board-sync` | `forge-board-sync.ts` (207) | PENDING | read/write → `db::forge_control`, command → `rust/cli/src/forge/` |
| 11 | `forge:sync-history` | `sync-forge-history.ts` (113) | PENDING | read/write → `db::forge_engine` |
| 12 | `forge:tools` | `forge-tools.ts` (135) | PENDING | tool catalog → `rust/forge/src/` (pure). VERIFY first: the static gate it runs may already be `forge harness-lint` |
| 13 | `story:status` | `set-story-status.ts` (138) | PENDING | the read exists (`db::forge_read::story_statuses`); the write is the missing half — a Rust command, not a TS script |
| 14 | `story:preflight` | `story-preflight.ts` (184) | PENDING | pre-run checks over `db::forge_read` + the packet lint |
| 15 | `sprint` | `sprint.ts` (229) | RETIRE (or PORT P2 if the sprint view returns) | reads deleted `lib/` modules |
| 16 | `health` | `sprint-cleanup.ts` (309) | RETIRE (or PORT P2) | the script is post-sprint hygiene — `.next` build dirs, stale worktrees, unpushed branches. Its subject is the retired Next.js tree (and worktrees, which AGENTS.md forbids), so the name outlived the work. `git status`, `pnpm broken:ts:sweep` and the pre-push hook cover what is left |
| 17 | `test:story`, `test:sprint-fences` | `test-story.ts` | PENDING (captain classifies) | **"run the fences, not the world"**: run exactly the assay commands a story, batch or sprint declares — de-duplicated — which the engine's QA does for itself but a human verifying afterwards could not. The Rust home is the engine's evidence gate (`rust/forge/src/engine/evidence_gate.rs`) plus a `forge` subcommand; `test:sprint-fences` is the same file with `--sprint` |

## Block B — agent runtime (3)

Not Forge, and not a port of its own: `rust/forge/src/bin/forge_worker.rs` is the documented replacement
for the loop these three drove. The inventory's verdict for the family is **RETIRE once the worker's
coverage is confirmed** (`docs/rust-parity-ledger.md` answers that).

| command | script | note |
| --- | --- | --- |
| `agent:runtime:dogfood` | `agent-runtime-invoke.ts` | one engine pass by hand; the worker does this with no TypeScript |
| `agent:runtime:deepseek` | `agent-runtime-deepseek.ts` | the DeepSeek harness; `rust/forge/src/engine/` owns agents and routing now |
| `agent:workspace` | `workspace-cli.ts` | **RETIRE, explicitly and permanently**: it created per-lane worktrees, which AGENTS.md forbids ("NO TREES. EVER."). Do not port it under any name |

## Block C — retired server stack (33)

These are not Forge work; each is a keep-or-delete decision on a name in the menu. Nothing here is
removed before the captain has read this page. The verdict vocabulary is the inventory's: **PORT**
(build it in Rust), **VERIFY** (likely already Rust — confirm before writing anything), **RETIRE** (do not
build it again; the file stays as reference).

### PORT — the capability is wanted and missing

| commands | script | Rust home, and who owns it |
| --- | --- | --- |
| `promote:warehouse:prod`, `:apply` | `promote-warehouse.ts` | the captain's 2026-09-27 decision: the warehouse stays alive with Apple sync, so this is DEV_OPS P0 and ports **together with the Apple mail promotion**, so the two share one promotion path. `scripts/contacts-sync.sh:135,143,149` calls it and the hop is down today |
| `mailbox:promote` | `promote-applemail.ts` | the same story — and the intake above it is dead too: `scripts/email-sync.sh:41` calls the banner-marked `apple-mail-envelope-intake.ts`, so the Apple Mail path is down at **both** steps (corrected 2026-09-28; the findings at the foot of this page already said so, this row did not) |
| `bank:load:dev`, `bank:load:prod`, `bank:dry-run` | `bank-transaction-load.ts` | accounting exists in `rust/core/domain/src/accounting.rs`; the statement loader is the missing half — a `rust/cli` loader, DEV/PROD explicit, `--dry-run` kept |
| `flight-recorder:qa-seed`, `:qa-reset` | `seed-flight-recorder-qa.ts` | DEV-only golden transaction; `rust/server/src/flight_recorder/` exists and the seed does not. Must fail closed on a PROD target |
| `db:export:projects` | `export-dev-projects-workspace.mjs` | its twin `db:seed:projects` is already Rust, so this is half-ported; the playbook says a Neon branch reset is the normal DEV refresh, and without this the reset is lossy. **PORT** (`db-tool export-projects`) |
| `whatsapp:coexistence:test` | `whatsapp-coexistence-completion.test.ts` | **PORT as a Rust test.** The implementation is not lost (integrations + `db/src/whatsapp.rs` + screens all exist); only its coexistence proof is stranded |
| `mq:worker`, `mq:worker:prod` | `mq-worker.ts` | **VERIFY then repoint or retire**: delivery is already Rust (`rust/server/src/mq_runtime.rs`), so either these names invoke the Rust runtime or they go, with a line in `docs/agent/MEMORY.md`. Do not re-implement the broker in TypeScript |
| `contacts:load:*`, `contacts:project:*` | `load-apple-contacts.ts`, `project-apple-contacts.ts` | **VERIFY** against `rust/cli/src/apple_sync.rs` (the `apple:sync` commands already target Rust). Likely RETIRE, or fold the batch behaviour into `apple_sync` — decide before porting |


### RETIRE — the subject is gone, or the playbook supersedes it

| commands | script | why |
| --- | --- | --- |
| `db:pull:dev` | `pull-prod-to-dev.mjs` | superseded: "pull PROD down to DEV" means a Neon branch reset from PROD (instant, byte-exact). A selective table-walk is the fallback path, not the normal one; if it is ever wanted it is a new `db-tool` subcommand |
| `cleanse:dev` | `cleanse-dev-fixtures.ts` | its job was to make a hand-loaded DEV usable; a DEV reset from PROD makes it moot |
| `verify:public-reads`, `verify:forms`, `verify:forms:e2e`, `verify:vault`, `verify:contract`, `verify:wbs`, `verify:bridge`, `verify:mapper` (8) | `verify-*-service.ts`, `verify-*-contract-*.ts`, `verify-listing-client-fill.ts` | DEV proofs of the **retired TypeScript services**. The capabilities are Rust (`rust/server/src/{vault,contracts,deals,forms,wbs}/`); where a proof is still worth having it belongs beside the Rust module it proves, as a test — never as a resurrected script |
| `probe:kind:dev` | `probe-kind-routing.ts` | the engine owns routing (`rust/forge/src/engine/routing_brain.rs`); a DEV probe of kind routing is superseded by the engine's own decision rows |
| `identity:phone:audit:prod`, `identity:phone:cleanup:prod`, `:apply` | `audit-phone-identities.ts`, `normalize-phone-identities.ts` | the normalisation **rules** are already Rust (`normalize_phone`, `domain/person.rs` identities); what these commands are is a PROD-MUTATING rewrite of canonical identity rows. RETIRE the names; if the cleanup is wanted it is a new PROD-safe Rust command under the captain's hand, not this script |
| `check:widgets` | `check-svar-widgets.mts` | RETIRE unless a Svar widget still ships; if one does, the gate points at whatever replaced `lib/forms/` |

## Findings this sweep produced

1. **A live shell script calls a dead TypeScript file.** `scripts/email-sync.sh:41` runs
   `apple-mail-envelope-intake.ts`, which carries the banner — and so do the `mailbox:verify` and
   `mailbox:intake` commands. `BROKEN-TS-INVENTORY.md` §APP 1 says that file "is not in the dead list and
   still runs"; it is, and it does not. The follow-on claim it supports ("inbound mail is not stranded")
   rests on a false premise, so the mail chain needs one read before the promotion port is scoped.
2. **`scripts/contacts-sync.sh` names three dead files** (`load-apple-contacts.ts`,
   `project-apple-contacts.ts`, `promote-warehouse.ts`) — the inventory already records this as "the
   warehouse promotion is down", and it is the same class of finding: live shell, dead TypeScript.
3. The three count corrections at the top of this page (56 → 53; `check:widgets` previously invisible).

## How to re-check this page

```
pnpm broken:ts:commands           # the three blocks and the count
pnpm broken:ts:commands --check   # fails if the count rose, or fell without the baseline being lowered
pnpm broken:ts:sweep              # the file-by-file counterpart, and the file count's own gate
```

Lower the baseline in `scripts/dead-command-sweep.mjs` and the number here when a command is ported or
removed — those two numbers are the whole metric, and they may only fall.

