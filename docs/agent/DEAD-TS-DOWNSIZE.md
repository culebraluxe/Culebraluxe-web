# Dead TypeScript — the downsize decision

**187 files are broken. 8 of them described work that still had to happen. One is now ported
(`apple-messages-intake.ts` → `rust/cli` apple-sync messages-intake); 7 are left to port. The other 179 are junk — not
translated, not kept alive, not paid for.** This page is the whole decision; the file-by-file list is
`BROKEN-TS-INVENTORY.md`, and `pnpm broken:ts:sweep` keeps the count honest.

## 1. Build these (Apple — green-lit)

| file | what it does, plainly | lines | verdict |
| --- | --- | --- | --- |
| `apple-messages-intake.ts` | reads the iMessage/SMS export, writes text threads into the client timeline | 365 | **PORTED** — `rust/cli/src/apple_messages.rs` + `domain::apple_messages` + `db::{RelationshipEvidenceDao, LandingDao}`; live check `scripts/rust-live-check/apple-messages-intake.mjs` |
| `apple-calls-intake.ts` | same for phone calls | 186 | **PORT** |
| `load-apple-contacts.ts` | reads the Apple Contacts export into the landing tables | 545 | **PORT** |
| `project-apple-contacts.ts` | landing tables → client records | 346 | **PORT** |
| `promote-warehouse.ts` | landing tables → `person` / `property` — **the hop with no Rust home at all** | 405 | **PORT** |
| `apple-mail-envelope-intake.ts` | Apple Mail headers → landing | 440 | **PORT** |
| `promote-applemail.ts` | mail landing → mail timeline | 193 | **PORT** |
| `gmail-metadata-sync.ts` | Gmail metadata → landing | 193 | **KEEP** — the captain's word, 2026-09-28: "yes fix email sync" (the Gmail path is the orphaned one) |

≈2,480 lines of TS become **one Rust job: four feeds in, one promotion out**. These are the five shell
scripts that already schedule them, so nothing new calls them into being —
`scripts/apple-sync.sh`, `contacts-sync.sh`, `apple-calls-sync.sh`, `email-sync.sh`, `gmail-sync.sh`.

## 2. Already in Rust — do not port, do not rebuild

Calendar intake and reminders (`rust/cli/src/apple_sync.rs`), outbound-to-Apple delivery (`drain` +
the event outbox), the mailbox service (`rust/core/service/src/mailbox.rs`, 1265 lines), WhatsApp,
website forms. `apple-gateway-worker.ts` (230 lines) is the outbound path → **KILL**, the Rust side
already exists. The Forge control plane is Rust too (`rust/forge/src/bin/forge.rs`, `forge_task.rs`,
`forge_worker.rs`), which is why ~20 `forge-*.ts` files are **KILL** — they were replaced, not deleted.

## 3. The other 179: KILL

Only two reasons, and both are final: **(a) already in Rust** (every `forge-*.ts`), or **(b) nobody needs
it** — 14 verifiers of a UI that no longer exists, the phone-identity one-offs, the bank transaction
loader, dev fixtures, story scaffolding, the runtime dogfood harness. None of it is translated.

**Two exceptions, and they matter — and both are already Rust (corrected 2026-09-28):**
`forge-packet-lint.ts` (663) and `forge-sync-agents.ts` (172) are the gates that keep AGENTS.md and the
vendor pointer files honest. They are no longer enforced by nothing: `rust/cli/src/forge/{lint,sync_agents,vendor_block}.rs`
replace them, `package.json` runs the Rust binary (`forge:packet-lint` → `forge harness-lint`,
`forge:sync-agents` → `forge sync-agents`), and both were run green on 2026-09-28. The earlier claim that they
"cannot run at all" was true of the deleted TypeScript and is now stale. Nothing to port here.

## 4. The menu is lying — 64 commands cannot run

64 of the 145 `package.json` commands point at a file that cannot load. Some you use by name:
`sprint`, `health`, `story:status`, `test:story`, `forge:doctor`, `forge:clean`, `forge:manifest`,
`db:pull:dev`, `verify:public-reads`, `verify:forms`, `promote:warehouse:prod`, `contacts:load:prod`,
`mailbox:intake`, `mailbox:promote`. A command that exists and cannot run is worse than no command.

## 5. Delete or keep?

The rule on the books is **marked, not deleted**, and it still makes sense: a dead file costs nothing at
rest, cannot be loaded by accident, and is searchable intent. The honest downsize is therefore **the
files stay, the lying menu entries go** — one mechanical pass that deletes the dead 64 and re-points the
ones that have a Rust equivalent at the Rust binary. If the files themselves should come off disk, that
is one `git rm` and this table is the list; nothing here is load-bearing.
