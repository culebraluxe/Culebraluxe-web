# Dead TypeScript — the downsize decision

**187 files are broken. 8 of them described work that still had to happen. Four are now ported
(`apple-messages-intake.ts`, the two Apple Mail files and `gmail-metadata-sync.ts` → `rust/cli`); 4 are left to
port. The other 179 are junk — not translated, not kept alive, not paid for.** This page is the whole decision; the
file-by-file list is `BROKEN-TS-INVENTORY.md`, and `pnpm broken:ts:sweep` keeps the count honest.

## 1. Build these (Apple — green-lit)

| file | what it does, plainly | lines | verdict |
| --- | --- | --- | --- |
| `apple-messages-intake.ts` | reads the iMessage/SMS export, writes text threads into the client timeline | 365 | **PORTED** — `rust/cli/src/apple_messages.rs` + `domain::apple_messages` + `db::{RelationshipEvidenceDao, LandingDao}`; live check `scripts/rust-live-check/apple-messages-intake.mjs` |
| `apple-calls-intake.ts` | same for phone calls | 186 | **PORT** |
| `load-apple-contacts.ts` | reads the Apple Contacts export into the landing tables | 545 | **PORT** |
| `project-apple-contacts.ts` | landing tables → client records | 346 | **PORT** |
| `promote-warehouse.ts` | landing tables → `person` / `property` — **the hop with no Rust home at all** | 405 | **PORT** |
| `apple-mail-envelope-intake.ts` | Apple Mail headers → landing | 440 | **PORTED** — `rust/cli/src/apple_mail.rs` (`apple-sync mail-intake`) + `domain::applemail` + `db::{AppleMailLanding, IntakeCheckpoint}`; the Python Envelope Index bridge is kept as the extractor (it is the only part that needs macOS TCC); live check `scripts/rust-live-check/apple-mail.mjs` |
| `promote-applemail.ts` | mail landing → mail timeline | 193 | **PORTED** — `apple-sync mail-promote` in the same Rust module: `l_applemail` → evidence → reconcile (`decide_apple_handle`) → `interaction` → read models |
| `gmail-metadata-sync.ts` | Gmail metadata → landing | 193 | **PORTED 2026-09-28** — `rust/cli/src/gmail.rs` (`gmail-sync`) + `domain::gmail` + `db::EmailLanding`; the captain's word was "yes fix email sync" (the Gmail path was the orphaned one). Needs `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` / `GOOGLE_REFRESH_TOKEN` in `.env.local` — absent on this machine, so only the fail-closed path is verified locally |

≈2,480 lines of TS become **one Rust job: four feeds in, one promotion out**. These are the five shell
scripts that already schedule them, so nothing new calls them into being —
`scripts/apple-sync.sh`, `contacts-sync.sh`, `apple-calls-sync.sh`, `email-sync.sh`, `gmail-sync.sh`.

**Two operational preconditions, 2026-09-28.** (1) The Mail extractor reads
`~/Library/Mail/<version>/MailData/Envelope Index`, which macOS gates behind Full Disk Access. Without
it the bridge exits 2 with a TCC message and `mail-intake` fails every account cleanly (non-zero exit,
nothing landed, nothing promoted) — the promotion half needs no such permission and was verified against
DEV. Grant Full Disk Access to the terminal/VS Code process, then fully restart it.
(2) `gmail-sync` (the Gmail API path) needs `GOOGLE_CLIENT_ID` / `GOOGLE_CLIENT_SECRET` /
`GOOGLE_REFRESH_TOKEN` (the `gmail.readonly` scope) in `.env.local`, which are deliberately unset — see the
long comment at `.env.local:192-200`, whose claim is that *"Gmail arrives through the authenticated Apple
Mail bridge instead"*. **Measured on this Mac, 2026-09-28: for Gmail that claim is not true.** Both Gmail
accounts deliver nothing into `INBOX` — their whole recent traffic sits in `[Gmail]/All Mail`
(`penfield33@gmail.com`: All Mail 3091 / Spam 757 / INBOX 0 recent; `culebraluxe@gmail.com`: All Mail 1148 /
Trash 143 / INBOX 0 recent), while the IMAP accounts behave (`lisa@culebraluxe.com` INBOX 128 + Sent 88 →
120 landed). The bridge reads INBOX + Sent and nothing else, on purpose: All Mail repeats one message once
per label, and the extractor's own history (see `normalize_message_id_header`) is a bug caused by exactly
that duplication. So **Gmail is currently captured by neither path** — the bridge sees no Gmail in INBOX,
and the API job has no credentials. Whichever way out is chosen (mint the Gmail credentials, or teach the
bridge to read All Mail with the global-message-id identity it already has), the account list in
`MAIL_APP_ACCOUNTS` is also wrong today: it names the two Gmail accounts, which yield zero, and omits
`lisapenfield@icloud.com` (INBOX 460 recent), which would yield real mail.

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

### 5.1 The first `git rm`, 2026-10-02 — the agent plane's test suite

`legacy/agent-runtime` (40 files), `legacy/services` (4) and `legacy/lib` (1) came off disk: **45 files,
6,643 lines**, every one of them a `*.test.ts` for code that was already deleted. They went first because no
argument for keeping them survived measurement.

- **No gate was watching them.** `pnpm broken:ts:sweep` scans `scripts` and `agent-runtime`; there is no
  top-level `agent-runtime/` — the harness lives under `legacy/` — so the sweep never opened them. Sweeping
  `legacy` as a root for the first time measures **465 files scanned, 431 cannot load, 0 marked, exit 1**:
  the gate's green was always scoped to `scripts/`, and the tree this page calls "marked, not deleted" is
  neither — 431 of its files are broken and unbannered.
- **They owned 379 of `tsc`'s errors.** `pnpm typecheck` measured on this machine (no `node_modules`, so the
  number is this machine's): **811 → 432**. What is left is `legacy/testv2` (384 error sites) and
  `legacy/workflow_app/forge` (48), so the step `gates.yml` removed on 2026-09-28 comes back when the last two
  dead trees go — not before.
- **§1 above is stale.** All four "PORT" files (`apple-calls-intake.ts`, `load-apple-contacts.ts`,
  `project-apple-contacts.ts`, `promote-warehouse.ts`) are already absent from `git ls-files`, and the Apple
  chain's promotion hop is Rust (`rust/cli/src/apple_contacts.rs`, `rust/cli/src/apple_mail/promote.rs`).
  "Marked, not deleted" existed to preserve intent for work not yet ported; that work is done, or its files
  are gone.

The retirement is a **property** now, not an event: `arch_boundary__012__retired_ts_trees_stay_retired` fails
if any of the three paths comes back, and if any live JavaScript or TypeScript imports anything under
`legacy/` — the rule this page states and nothing enforced (`eslint.config.mjs` names the paths in a banned
group, and CI runs no eslint step; `gates.yml:7-8` is the repository's own note that an unenforced rule is
not a rule).

**The budget that remains is 74,977 lines:** `legacy/workflow_app` (66,687, of which 66,386 is `tests/`) and
`legacy/testv2` (8,342). The deletion is the same one `git rm`; what it needs first is the captain's answer to
two questions this measurement opens — are the TypeScript tests still the reference the port is checked
against, and should the 431 broken-unbannered files be bannered (what the sweep's contract expects) or
deleted (what this page would prefer)?
