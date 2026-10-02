# Broken TypeScript inventory — translate to Rust, never revive

> **Short version first: `DEAD-TS-DOWNSIZE.md` (same directory) is the decision.** 186 files are broken.
> The Apple chain that still owed work is ported (Contacts closed 2026-09-28; see §"Live callers"), so
> what is left is 2 Forge gates plus files that are killed outright — not translated, not maintained.
> This page is the file-by-file reference behind that page.
>
> **Deleted 2026-10-02:** `legacy/agent-runtime`, `legacy/services` and `legacy/lib` came off disk — 45
> files, 6,643 lines, every one a `*.test.ts` (§5.1 of that page). The appendix rows below that name them
> are the 2026-09-27 record, not a description of the tree. Measured while removing them: no root of
> `pnpm broken:ts:sweep` covers `legacy/` at all, and `legacy/` holds **431 files that are broken and carry
> no banner** — so "marked, not deleted" was true of the files this page lists and never of that tree.
>
> **Archived 2026-10-02, the same day:** the rest of the tree went with them — 432 files, 79,479 lines, out of
> the tree to `~/Documents/forge-legacy-ts-archive-2026-10-02/` (byte-identical copy plus a zip, taken at
> `5b4e5aa4`). This page is therefore the **historical record** of what was broken and why — not a description of
> anything on disk. `DEAD-TS-DOWNSIZE.md` §5.2 has the counts and the restore path.


## The rule (read this before touching any file in `scripts/`)

Every file listed here carries this banner as its first comment:

```
⚠ BROKEN ON PURPOSE — DO NOT FIX, DO NOT IMPORT, DO NOT CALL, DO NOT REVIVE.
```

It is not a to-do. It is a property of the file. The TypeScript engine and its libraries (`lib/`,
`agent-runtime/`, `legacy/db/`) were deleted in the 2026-09 Rust port, so these files cannot load:
their imports resolve to nothing. They are kept **as reference only** — a description of behaviour
that was once real, so that behaviour can be **translated into Rust** if it is wanted again.

This repository had a specific, expensive failure mode, and the banner exists to stop it:
agents reached across the branch boundary and re-integrated **live production code with retired
TypeScript libraries**, which is why the Node/TypeScript engine had to be dropped entirely.
Reading one of these files for its intent is fine. Wiring live code to it is not.

- **Translate, never revive.** The Rust home is the destination, never a resurrected TS file.
- **A dead `pnpm` command is not runnable.** At the 2026-09-27 measurement, 64 of the then-145 `package.json`
  scripts named a file listed here (today's number is `pnpm broken:ts:commands`: **10 of 107**). Those commands
  exist in the menu and cannot run; that is a known, marked state —
  and some of them are the gates and the sync jobs (see "Live callers" below), not conveniences.
- **`agent:workspace` must never be ported.** `scripts/workspace-cli.ts` created per-lane
  worktrees, and AGENTS.md forbids that outright — "NO TREES. EVER. There is ONE workflow and it is
  the rows". It is listed here for completeness, and its verdict is RETIRE, not port.

## How this list was produced, and how to re-check it

`pnpm broken:ts:sweep` walks `scripts/` and `agent-runtime/`, resolves every import statically, and **fails**
when a broken file is unmarked or a marked file still loads — so the list and the tree cannot drift apart
silently. It touches no database and no network.

**It is Rust now, and it is the same gate.** Ported 2026-09-29 to `rust/cli/src/forge/ts_sweep.rs`
(`cargo run -p cli -- forge ts-sweep`), because the port that removed the last Node script in this repository
could not leave its own metric behind. The TypeScript it replaced printed the same four counts and the same
drift sections, diffed line for line before the file was deleted; the one deliberate change is that the drift
lists print **sorted**, since `readdir` order is the filesystem's and a gate whose output moves cannot be
diffed. CI runs it from the `rust` job (`.github/workflows/gates.yml`), which is where cargo is.

Measured the day of the port, on the tree it landed on: **10 files scanned, 0 cannot load, 0 cannot work,
0 marked — the two roots hold nothing broken and unmarked, which is the one thing this gate exists to say.**
(An hour earlier the same gate said 37 / 15 / 9 / 24: both lanes are deleting at once, so the count belongs to
the command and never to this page.) The table below is the 2026-09-27 measurement and is kept as the record of
that day, not as the current count — the command is the authority.

Corrected **2026-09-27**, after the first sweep was found to undercount. The first pass did not walk the
whole tree, and it marked 9 files that load fine. Measured by the resolver:

| class | count | meaning |
| --- | --- | --- |
| **CANNOT LOAD** | **172** | a *value* import resolves to nothing, directly or through another broken file. `import type` does not count: tsx erases it, so it cannot break loading. |
| **CANNOT WORK** | **14** | the file loads, but a *lazily imported* module it needs is gone — it parses and cannot do its job. |
| marked | 186 | every file in both classes carries a banner, and nothing else does. |

That is **172 of the 247** TypeScript files under `scripts/` (197) + `agent-runtime/` (49). `legacy/db/`
is gone entirely; `agent-runtime/` still exists but 20 of its 49 files cannot load, and all of them are
now marked.

`lib/` today contains only `forms/` and `trust-ui/`. Any `../lib/...` import outside those two subtrees is
permanently unresolvable.

## Live callers — what is actually down right now

A marked file is inert until a **live** caller invokes it. Six scheduled/operator paths do, so these
capabilities are not "dead weight", they are **broken in production** (checked 2026-09-27):

| live caller | calls | consequence |
| --- | --- | --- |
| `scripts/apple-sync.sh` — launchd `com.culebraluxe.apple-sync`, twice daily | **`rust/cli` apple-sync messages-intake** (was `scripts/apple-messages-intake.ts`) | **fixed 2026-09-27**: the launchd job's exit status 1 was the deleted intake script; the step is Rust now and the job needs re-arming (see §"Apple intake — ported") |
| `scripts/apple-message-repair.sh` (`apple:repair:prod`) | **`rust/cli` apple-sync messages-intake --evidence-only --refresh** (was the same deleted file) | **fixed 2026-09-27**: repairs ODS evidence for the existing export and refreshes the Client read models, without replaying interactions |
| `scripts/contacts-sync.sh` | **`rust/cli` apple-sync contacts-load / contacts-project / warehouse-promote** (was `load-apple-contacts.ts`, `project-apple-contacts.ts`, `promote-warehouse.ts`) | **fixed 2026-09-28: the whole Contacts chain is Rust.** All three steps are database functions in `db/migrations/254_apple_contacts_load_project.sql` (load, projection) and `253_apple_contacts_promote.sql` (promotion), called from `rust/cli/src/apple_contacts.rs` and repointed at `contacts-sync.sh:148,156,163`. DEV-verified: load replay 2855/2855 with 0 changed, projection before=after=2855, and `apple_contacts_fingerprint_audit()` re-derives **4794/4794** historical fingerprints — the ODS does not churn. `scripts/promote-warehouse.ts` is **deleted**; `load-apple-contacts.ts` / `project-apple-contacts.ts` stay bannered as reference and must never be revived |
| `scripts/apple-calls-sync.sh:41` | **`rust/cli` apple-sync calls-intake** (was `scripts/apple-calls-intake.ts`) | **fixed 2026-09-28**: the wrapper is repointed; chain ported to Rust (`rust/cli/src/apple_calls.rs` + `domain::apple_calls`), DEV-verified (5236 calls replayed, 864 evidence rows, 4 interactions written / 403 already current) |
| `scripts/email-sync.sh` | **`rust/cli` apple-sync mail-intake + mail-promote** (was `apple-mail-envelope-intake.ts` + `promote-applemail.ts`) | **fixed 2026-09-28**: the wrapper is repointed; intake needs macOS Full Disk Access (see `DEAD-TS-DOWNSIZE.md` §1) |
| `scripts/gmail-sync.sh` | **`rust/cli` gmail-sync** (was `scripts/gmail-metadata-sync.ts`) | **fixed 2026-09-28**: repointed; needs GOOGLE_CLIENT_ID / SECRET / REFRESH_TOKEN in `.env.local`, which this machine does not have |

**The promotion hop exists now, and it is not a script.** `warehouse_promote_apple_contacts(p_apply)`
(`db/migrations/253_apple_contacts_promote.sql`) does the whole landing → warehouse transformation
set-based inside Neon, called by `apple-sync warehouse-promote` (`rust/cli/src/apple_contacts.rs`) and
repointed in `contacts-sync.sh:163`. `scripts/promote-warehouse.ts` is **deleted** (2026-09-28) — do not
re-instate it. Its siblings for the load and the projection got the same treatment the same day:
`apple_contacts_load(jsonb, text)` and `apple_contacts_project(text, uuid)` in
`db/migrations/254_apple_contacts_load_project.sql`, called by `apple-sync contacts-load` /
`contacts-project`. A database function with a thin caller, never a script that pulls rows out to mutate them.

**Operator commands that cannot run and are named in AGENTS.md or used daily** — corrected 2026-09-28, because
the Forge half of this list was stale: `forge:doctor`, `forge:clean`, `forge:story:reset`, `forge:packet-lint`,
`forge:manifest`, `forge:sync-agents` and `forge:harness` **all run**. `package.json` points them at the Rust CLI
(`rust/cli/src/forge/`), `pnpm broken:ts:commands` counts none of them among the 53 dead names, and both gates were
run green on 2026-09-28. Still dead and used daily: `sprint`, `health`, `test:story`, `test:sprint-fences`,
`story:status`, `story:preflight`, `db:pull:dev`, `probe:kind:dev`. A runbook step that
cannot execute is a finding, not a footnote.


## What is ALREADY in Rust — do not re-port these

Verified by path, not assumed. Most of the **product** survived the port; what is missing is mostly
**tooling** (one-shot loaders, proofs, promotions, harness gates).

- Apple — **partly, and the doc previously overstated this.** What exists: `rust/cli/src/apple_sync.rs`
  (380 lines — `drain`, `calendar-intake`, `reminder-intake`). What does NOT exist: any Rust
  Messages/Calls/Contacts intake. `rust/integrations/src/apple/mod.rs` is a **4-line placeholder comment**
  ("native Swift/EventKit helpers can remain native adapters"), not an implementation, so the Apple
  messages/calls/contacts *pipelines* are still the dead TS scripts named by the live `*-sync.sh` callers.
- Mail: `rust/integrations/src/mail/mod.rs` (330 lines), `rust/integrations/src/bin/mail_test.rs`,
  `rust/core/service/src/mailbox.rs` (1265 lines)
- WhatsApp — **verified by reading the code on 2026-09-27, not by grep**. The captain's port is real
  and complete: `rust/integrations/src/whatsapp/mod.rs` owns the Meta Cloud API trust boundary
  (the `X-Hub-Signature-256` HMAC over the exact raw body, compared with `subtle::ConstantTimeEq` —
  not `==`; `from_env` refuses a missing secret; `normalize_e164` validates the owned number),
  `whatsapp/payload.rs` (387 lines) parses the webhook into a normalized event,
  `rust/core/db/src/whatsapp.rs` (736 lines) has `land`, `process_event` and
  `refresh_client_read_models`, `rust/server/src/whatsapp.rs` has `verify_handshake` and
  `handle_webhook`, and `rust/ui/src/app/screens/whatsapp_meta.rs` + `whatsapp_public.rs` are the
  screens. **The security half is also Rust**, in `rust/server/src/security/`: `mod.rs` (540 lines)
  carries `resolve_identity`, `decide`, role entitlements, `get_principal`, `warm_identity_cache`,
  plus `audit.rs`, `entitlements.rs`, `entitlement_catalog.rs`, `guest.rs`, `identity_cache.rs`
  (~1900 lines total). Nothing about WhatsApp or its authorization is stranded in TypeScript.
- Intake: `rust/core/db/src/intake.rs`, `rust/core/domain/src/intake.rs`, `rust/server/src/intake.rs`
- Relationship evidence: `rust/core/db/src/relationship_evidence.rs`,
  `rust/core/domain/src/relationship_evidence.rs`, `rust/server/src/relationship_evidence.rs`
- Calendar / catch-up: `rust/server/src/calendar/`, `rust/server/src/showings/`
- Communications / Gmail: `rust/core/domain/src/comms.rs`, `rust/core/domain/src/client.rs`,
  `rust/server/src/communications/`
- Accounting / bank: `rust/core/domain/src/accounting.rs`, `rust/ui/src/app/screens/accounting/`
- Flight recorder: `rust/server/src/flight_recorder/`
- CRM / clients / people / projects / deals / contracts / firms / properties / media / forms:
  the matching `rust/server/src/` and `rust/core/domain/` modules
- MQ delivery: `rust/server/src/mq_runtime.rs`
- The Forge engine itself: `rust/forge/src/` — six roles (`roles/scout|architect|lead|smith|qa|dev_ops`),
  the engine (`engine/dispatch`, `graph`, `split_join`, `spend_cap`, `evidence_gate`, `git_publish`,
  `xml`, `agents`, `routing_brain`, `db_writer`), plus `release/`, `execution/`, `routing/`, `evidence/`
  and the binaries `forge`, `forge_worker`, `forge_task`, `re_workflow`
- The website transport: `rust/server` + `lib/rust-api/` (thin client only)
- The cockpit and screens: `rust/ui` (Yew MVI)
- DB gates: `rust/cli/src/db_tool.rs` — applier, ledger report, DEV/PROD parity gate

**Not found in Rust at all** (grep over `rust --include=*.rs` returns nothing): the `warehouse`
promotion, the `db-zombies` cleaner, and every Forge *harness* tool (`packet-lint`, scope manifest,
`sync-agents`). Those are the real gaps, which is why they lead the priority list.

**Decision taken 2026-09-27 (captain): the warehouse stays alive, with Apple sync.** The
L-tables → Warehouse promotion is **PORT**, not RETIRE — it is a live hop with no implementation, not
a dead experiment, and it is ported together with the Apple mail promotion so the two share one
promotion code path.

Verdict vocabulary: **PORT** (build it in Rust) · **VERIFY** (likely already Rust — confirm coverage
before writing anything) · **RETIRE** (do not build it again; the file stays as reference).

---

# 1. DEV_OPS — priority order

## P0 — highest consequence first

1. ~~`scripts/promote-warehouse.ts`~~ — **DONE 2026-09-28: the file is deleted** and the hop is
   `warehouse_promote_apple_contacts(p_apply)` (`db/migrations/253_apple_contacts_promote.sql`) called by
   `apple-sync warehouse-promote prod [--apply]`; run on PROD (2855 landing rows → 2814 matched, 11 places
   created, 11 linked). The captain's decision (2026-09-27) was to rebuild the hop, not the script, and
   that is what landed. The load and the projection followed the same day
   (`db/migrations/254_apple_contacts_load_project.sql`).
2. `scripts/promote-applemail.ts` — `mailbox:promote` — `l_applemail` → Warehouse. Same shape,
   same gap, same decision: **PORT**, sharing the promotion code from 1 (one promotion path, two
   sources). Inbound mail exists (`integrations/mail`, `service/mailbox.rs`); promotion does not.
   See APP 1 below: the *live* intake is not this file's sibling — it is `apple-mail-envelope-intake.ts`,
   which still runs.
3. `scripts/export-dev-projects-workspace.mjs` — `db:export:projects` — captures the DEV Projects
   workspace so a DEV refresh can restore it. Its twin `db:seed:projects` is already Rust, so the
   pair is half-ported. **PORT** (`db-tool export-projects`): without it, a DEV branch reset is
   lossy, and the playbook says branch reset is the normal refresh path.
4. `scripts/pull-prod-to-dev.mjs` — `db:pull:dev` — selective table-walk PROD → DEV.
   **RETIRE** under the current playbook (a Neon branch reset from PROD is instant and byte-exact);
   if a selective pull is ever needed, it is a new Rust subcommand, not this file.
5. `scripts/mq-worker.ts` — `mq:worker`, `mq:worker:prod` — the broker poller entry point, "ONE
   dispatch pass". The delivery runtime **is** in Rust (`rust/server/src/mq_runtime.rs`), so this is
   the one dead command whose capability already exists. **VERIFY then repoint or retire**: either
   `mq:worker` invokes the Rust runtime or the npm names go away with a note in MEMORY.md. Do not
   re-implement the broker in TypeScript.

## P1 — operator tools worth having

6. `scripts/db-zombies.mjs` — find and terminate zombie/lingering backends. Not in Rust anywhere.
   **PORT** (`db-tool zombies`, terminate behind an explicit flag) — the pool work in
   `rust/core/db/src/pool.rs` makes this the natural companion tool.
7. `scripts/import-property-photos.mjs`, `scripts/import-property-document.mjs`,
   `scripts/import-missing-guide-images.mjs`, `scripts/import-guide-images.mjs` — the four media
   importers. **PORT** as ONE Rust importer that goes through `media` and `property_media`
   (AGENTS.md: media is the reusable asset, `property_media` owns role and order). Never
   special-case a listing.
8. `scripts/seed-flight-recorder-qa.ts` — `flight-recorder:qa-seed`, `flight-recorder:qa-reset` —
   DEV-only durable QA golden transaction. `rust/server/src/flight_recorder/` exists; the seed does
   not. **PORT** (must fail closed on a PROD target).
9. `scripts/seed-projects-mvi2-dev.ts` — **VERIFY** then PORT P2: projects live in
   `rust/server/src/projects/`; a seed that only exists to fill a DEV screen may be **RETIRE**.

## P2 — historic one-shots: RETIRE

10. `scripts/migration-ledger-baseline.mjs` — ran once (2026-09-10), superseded by the ledger and
    `db-tool status`. **RETIRE.** Note the gap this story found: DEV's ledger was missing
    `144_schema_migration_ledger.sql`, so the baseline backfill was thinner than the playbook claimed.
11. `scripts/fix-person-name-order-julio-pimentel-ortiz.mjs` — one-off data correction, applied.
    **RETIRE.**
12. `scripts/sprint.ts`, `scripts/sprint-cleanup.ts` — **RETIRE**, or PORT P2 if the sprint view is
    wanted again (they read deleted `lib/` modules).
13. `scripts/workspace-cli.ts` — **RETIRE, explicitly and permanently.** It created per-lane
    worktrees; AGENTS.md forbids that ("NO TREES. EVER."). Do not port it under any name.
14. `scripts/check-svar-widgets.mts` — `check:widgets`. **RETIRE** unless a Svar widget still ships,
    in which case re-point the gate at whatever replaced `lib/forms/`.
15. `scripts/generate-form-review-pdf.ts` — **PORT P2** if form-review PDFs are still a deliverable
    (`lib/forms/` survives, so this is one of the few files whose import could still resolve — fixing
    it in TS is still out of scope). Otherwise **RETIRE**.
16. `scripts/rust-api-client.test.ts`, `scripts/rust-ui-mount.test.ts` — **RETIRE.** Their subjects
    are `rust/server` and `rust/ui`, and they are tested there now.

---

# 2. FORGE — priority order

The engine itself is already Rust (`rust/forge/src/`). What is dead here is the **harness around
it**: the lints, the manifest, the board feeders. Two of them are load-bearing gates.

## P0 — gates that currently enforce NOTHING

### What these two things actually are (plain English, because "lint" is a misleading name)

A **packet** is `docs/agent/packets/<STORY-ID>.md` — the per-story work order an agent reads before
editing. **`forge:packet-lint` is not a helper that tidies text. It is the checker that refuses the
work:** it fails when a packet cites evidence as prose instead of a path and a line range
(`scripts/forge-packet-lint.ts:157-160`), when that path no longer exists, when the range runs past
the end of the file, when a manifest row points at a file that is gone, or **when the guardrail block
in AGENTS.md has drifted away from what the vendor pointer files (`CLAUDE.md`, Cursor, Warp) say.**
It is the thing that says NO.

**`forge:sync-agents` is the other half of the same pair:** it re-renders that managed guardrail
block from ONE source (`lib/agent-vendor-block.ts`) into the vendor files, so nobody retypes a rule
and gets it subtly wrong. `pnpm forge:harness` runs sync `--check`, the manifest check and the lint
in sequence.

**Why it is P0:** both files import deleted modules (`lib/agent-vendor-block`, `lib/scope-manifest`),
so **both commands fail to load** — today the drift guarantee is enforced by nothing but an agent
remembering. A gate that cannot run is worse than no gate, because the pipeline still reports green.

**Is it DEV_OPS or Forge?** It is neither the engine nor the product. It is **harness plumbing that
sits next to the engine and never touches it**: it did not port, and must not change, anything about
roles, dispatch, the graph, spend or evidence in `rust/forge/src/`. The captain's port of the engine
is not at risk here; this is the paperwork machine that reads packets and vendor files. Verdict:
**PORT**, and it belongs in `rust/` with the rest of the tooling so it is one toolchain, not two.

1. `scripts/forge-packet-lint.ts`, `scripts/forge-packet-lint.test.ts` — `forge:packet-lint`.
   Enforces the packet rules, evidence-as-path-and-line-range (rule 10), and the drift check between
   AGENTS.md and the vendor pointer files. The file imports deleted `lib/agent-vendor-block` +
   `lib/scope-manifest`, so **`pnpm forge:packet-lint` cannot run and the guarantee is unenforced.**
   **PORTED 2026-09-27** — `rust/cli` `forge harness-lint` (`rust/cli/src/forge/lint.rs` and its
   helpers), with 39 must-fail/must-pass fixtures — "a gate nobody has seen fail is indistinguishable
   from a gate that cannot fail" is written in the file itself, and it stays true. The four deleted
   `lib/` modules it depended on (`agent-vendor-block`, `scope-manifest`, `forge-decision`,
   `secret-shapes`) were recovered from `4cf98110^` and translated into
   `rust/cli/src/forge/{vendor_block,citations,decision,secret_shapes}.rs`.
2. `scripts/forge-sync-agents.ts`, `scripts/forge-sync-agents.test.ts` — `forge:sync-agents`.
   Renders the managed guardrail block into vendor pointers. **PORTED 2026-09-27, in the same change
   as 1** — `rust/cli` `forge sync-agents` (`rust/cli/src/forge/sync_agents.rs`); the four load-bearing
   rules and the backing AGENTS.md sentences come from ONE Rust module (`forge::vendor_block`), so lint
   and sync cannot disagree. Proof of the translation: `forge sync-agents --check` reports the existing
   `CLAUDE.md` block `ok` — a byte-identical match with the block the TypeScript renderer wrote.
3. `scripts/forge-manifest.ts`, `scripts/forge-manifest.test.ts` — `forge:manifest`,
   `forge:manifest:check`. **PORTED 2026-09-28** — `rust/cli` `forge manifest`, with the rules in
   `rust/forge/src/scope_manifest.rs` (lanes, TF-IDF over the restricted corpus, rendering, drift,
   the write refusal — pure, no filesystem, no git, no clock) and the gather-and-print half in
   `rust/cli/src/forge/manifest.rs`. The two `lib/` modules it needed were recovered from `4cf98110^`
   and translated: `lib/scope-manifest.ts` → `forge::scope_manifest`, `lib/git/sync-conflict.ts` →
   `forge::sync_conflict`. It is the writer half of a pair — the packet lint's rule 8 parses these rows
   and fails on a row that resolves nowhere — and it was the second link of `pnpm forge:harness`, so the
   whole harness chain was dead at that link. One deliberate deviation, measured: the header's
   `generated:` clock is compared separately from the body, because `--check-all` (which the harness
   chain runs) otherwise rewrote eight committed manifests on EVERY run and left a dirty worktree.

## P1 — board and engine feeders

4. `scripts/forge-story-reset-config.ts` — pure argv/env resolution, side-effect free by contract.
   **PORT P1** as a small module beside `db_tool`; the reset path is used every story.
5. `scripts/forge-handoff.mjs` — "how a Forge role states its decision now". **VERIFY** first: the
   Rust engine owns decisions (`engine/decisions.rs`, `engine/db_writer.rs`,
   `engine/evidence_store.rs`). If it does, **RETIRE**; if not, **PORT P1**.
6. `scripts/forge-human-gate-pass.mjs` — **VERIFY** then PORT P1 (the engine has gate code).
7. `scripts/forge-test-stories.ts`, `scripts/forge-record-stories.ts`, `scripts/forge-ladder.ts` —
   the board feeders that drive the engine and leave real work behind. **PORT P1/P2**, one Rust
   `forge` subcommand each, DEV/PROD-explicit.

## P1 — the agent loop: RETIRE after verifying the Rust engine

8. `scripts/agent-work.ts`, `scripts/agent-work-entry.ts`, `scripts/agent-runtime-invoke.ts`,
   `scripts/agent-runtime-deepseek.ts`, `scripts/forge-orchestrate-wake.ts`,
   `scripts/forge-runtime-recover.ts`. `agent-runtime/` is deleted;
   `rust/forge/src/bin/forge_worker.rs` is already the documented replacement for
   `agent-work-entry.ts`; the loop now lives in
   `rust/forge/src/engine/{runtime,opencode,agents}.rs`. **RETIRE** once the worker's coverage is
   confirmed (see "Verification debt" below).

## P2 — historic board writes: RETIRE

9. `scripts/forge-batch-release.mjs`, `scripts/forge-holes-board.mjs`,
   `scripts/update-core-daily.mjs`, `scripts/update-core-daily-2.mjs`,
   `scripts/update-core-daily-0910.mjs`, `scripts/update-rel-intel-stories.mjs`,
   `scripts/rel-intel-nav-close.mjs`, `scripts/projects-workspace-board.mjs`,
   `scripts/projects-workspace-scope-note.mjs` — one-shot board loads and normalizations, already
   applied; the Rust engine writes the board now, and `pnpm forge:clean` /
   `pnpm forge:story:reset` are the live equivalents. **RETIRE.**

---

# 3. APP — priority order

The product is largely Rust already. What is stranded here is intake and proof tooling.

## P1 — intake production still depends on

1. `scripts/apple-mailbox-intake.ts` — an **older** mailbox intake into the L table. **RETIRE,
   superseded — corrected 2026-09-27 by reading `package.json`.** `mailbox:intake` and
   `mailbox:verify` do not name this file; they name `scripts/apple-mail-envelope-intake.ts`, which
   is **not in the dead list and still runs**. So inbound mail is not stranded: the live intake is
   alive, and the only missing half is **promotion** — which is DEV_OPS 2, not this file. The
   dead-list count is unchanged; only this file's verdict is.
2. `scripts/apple-messages-intake.ts` (+ `-proof`, `-real-load`) — iMessage → ODS. **PORTED 2026-09-27.**
   `rust/cli/src/apple_messages.rs` (`apple-sync messages-intake <export-dir>`), rules in
   `rust/core/domain/src/apple_messages.rs` (11 unit tests, fingerprint verified against the deleted
   TypeScript's own output), writes through `db::{RelationshipEvidenceDao, LandingDao}`:
   evidence upsert → deterministic reconcile (`record_decision`) → `l_imessage` landing (500-row
   batches) → `latest:<person>:<channel>` interaction → client read-model refresh. Verified live on
   DEV: `node scripts/rust-live-check/apple-messages-intake.mjs` (18 checks). `scripts/apple-sync.sh`
   now calls the Rust command and needs neither node nor tsx. **The live check found a real bug** —
   serde's camelCase read the exporter's `dateISO` as `dateIso`, so every message silently lost its
   timestamp; fixed and pinned by a unit test.
3. `scripts/apple-calls-intake.ts` — calls channel. **PORTED 2026-09-28.**
   `rust/cli/src/apple_calls.rs` (`apple-sync calls-intake [dev|prod] --file <calls.jsonl>`), rules in
   `rust/core/domain/src/apple_calls.rs` (6 unit tests): FaceTime decided by provider or call type,
   directions derived from `originated`, evidence per counterparty address with `l_call` landing
   (`db::LandingDao::land_call_batch`, 500-row batches, source precision kept), reconciliation
   through the shared `decide_apple_handle`, one interaction per Person x call channel
   (`latest:<person>:call`, carrying duration). `scripts/apple-calls-sync.sh` now calls the Rust
   command. Verified live on DEV (replay-safe: 5236 replayed, 0 landed; 864 evidence rows; 4
   interactions written, 403 already current, 568 calls staged as evidence only). **The live check
   found the same bug as the Messages port** — the deleted TypeScript's `dateISO` key read as
   `dateIso` meant every call lost its date; fixed and pinned by a unit test.
4. ~~`scripts/load-apple-contacts.ts`~~ — **DONE 2026-09-28**: `apple_contacts_load(jsonb, text)`
   (`db/migrations/254_apple_contacts_load_project.sql`) called by `apple-sync contacts-load`; the file
   stays bannered as reference. The projection (`project-apple-contacts.ts`) landed the same way as
   `apple_contacts_project`.
5. `scripts/bank-transaction-load.ts` — statement load. Accounting exists in
   `rust/core/domain/src/accounting.rs`; the loader may not. **PORT P1.**
6. `scripts/gmail-metadata-sync.ts`, `scripts/rel-intel-load-gmail.ts` — bounded Gmail census and
   metadata sync through the neutral ODS seam. **VERIFY** `rust/integrations/src/mail/` +
   `rust/core/domain/src/comms.rs`, then **PORT P1** the missing half.
7. `scripts/whatsapp-coexistence-completion.test.ts` — **PORT P1 as a Rust test.** This answers the
   "was WhatsApp lost?" question: the implementation is NOT lost —
   `rust/integrations/src/whatsapp/`, `rust/core/db/src/whatsapp.rs`, `rust/server/src/whatsapp.rs`
   and the `whatsapp_meta` / `whatsapp_public` screens all exist. Only its coexistence test is
   stranded.

## P2 — proofs and verifiers: RETIRE, or move next to the Rust code

8. `scripts/verify-crm-intake.mjs`, `verify-crm-foundation.mjs`, `verify-crm-email-intake.mjs`,
   `verify-crm-communications-intake.mjs`, `verify-crm-person-creation.mjs`,
   `verify-website-intake.mjs`, `verify-website-intake-general-enquiry.mjs`,
   `verify-needs-review-resolution.mjs`, `scripts/core-daily-proof-12-13.ts`,
   `scripts/rel-intel-proof-opps.ts`, `scripts/rel-intel-proof-readmodel.ts` — DEV proofs that a
   capability persists correctly. The capabilities are in
   `rust/server/src/{intake,clients,people,deals,relationship_evidence}.rs`. **RETIRE**, or PORT P2
   as Rust integration tests that live beside the module they prove.
9. `scripts/calendar-eventkit-intake.ts`, `scripts/eventkit-seam-proof.ts`,
   `scripts/catchup-calendar-proof.ts`, `scripts/catchup-calendar-attention-proof.ts`,
   `scripts/catchup-dev-proof.ts` — **VERIFY** `rust/server/src/calendar/`; keep at most one Rust
   seam proof (the EventKit round trip is the kind of seam a unit test cannot see).
10. `scripts/verify-contract-mapper.ts`, `scripts/verify-listing-client-fill.ts`,
    `scripts/verify-listing-contract-bridge.ts` — **VERIFY** against `rust/server/src/contracts/`
    and `deals/`; likely already covered.

## Verification debt this inventory creates

- **RESOLVED 2026-09-27 by reading the code, not by grep:** the WhatsApp capability — including its
  Meta signature verification and its authorization (`rust/server/src/security/`, ~1900 lines with
  `resolve_identity`, `decide`, entitlements, `get_principal`, `warm_identity_cache`, `audit.rs`) — is
  **already Rust and complete**. Nothing about WhatsApp or its security is stranded in TypeScript.
  See the verified bullet in "What is ALREADY in Rust" above.
- **The Rust Apple Messages/Calls/Gmail intake channels are still claimed, not confirmed.**
  Confirming them is one read of `rust/server/src/intake.rs` + `rust/core/db/src/intake.rs`, and it
  decides whether APP 2–4 are PORT or RETIRE. This is the single cheapest read left in this document.
- The Rust Forge engine's coverage of the retired agent-loop scripts (FORGE 8) is likewise
  unverified. `docs/rust-parity-ledger.md` exists for exactly this question — read it before porting.
- Not debt, but written down 2026-09-27 so it is not rediscovered: the **production build/deploy
  and release set** (`pnpm build`, `build:all`, `deploy:prod`, `release`, `smoke:prod` and the
  `vercel-*` scripts) is documented in `docs/agent/DEV-OPS-RELEASE.md`.

---

# Sequencing — the stories, in the decided order (2026-09-27)

1. **FORGE P0** (`forge-packet-lint` + `forge-sync-agents` in Rust, one rule source). First because
   it is a **gate that is currently enforcing nothing**, and because the file that defines the
   guardrails is itself unreadable to the tool that replicates them. It does not touch the engine.
   **DONE 2026-09-27** — `rust/cli` `forge harness-lint` + `forge sync-agents`, one rule source in
   `rust/cli/src/forge/vendor_block.rs`. What the first live run taught: the gate is green on HEAD
   (0 failures) *because* it reports 175 baselined warnings — the 2026-09 port deleted the TypeScript
   tree and 22 packets/maps/manifests still cite it — and it also found that
   `docs/agent/harness-lint-baseline.json` had been CORRUPT JSON since it was written, which the
   TypeScript read as "no debt at all".
2. **DEV_OPS P0** (`db-tool` subcommands: the L→Warehouse promotion **including Apple mail**
   — decided PORT, not retire — plus `export-projects` and `zombies`). This is the highest-consequence
   dead code: a DEV refresh is lossy today, and the L→Warehouse hop has no live implementation at all.
3. **APP P1** (Apple channel completeness: messages/calls/gmail intake, then the promotion from 2).
   One read of `rust/{server,core/db}/src/intake.rs` first decides which channels already exist; the
   mailbox half is already answered — its intake runs, only promotion was missing.

Then: repoint or remove the dead `pnpm` names, and work the RETIRE lists down — the file count in
this inventory is the metric, and it may only fall.

# Maintaining this list

- The banner is inserted idempotently (it greps for its own marker), so re-running the sweep is safe.
- A new file that cannot load is a finding: either its capability moves to Rust, or it gets the
  banner. What it does not get is a repair in place.
- A **RETIRE** verdict is not permission to delete. The file stays as reference; deletion is a
  separate, explicit decision (as it was for the three gates replaced by `db-tool`).

---

# Appendix A — the original sweep (77 files), tagged

The first sweep's enumeration. Format: `path` — section → verdict. It is **partial**: Appendix B
carries the files this sweep missed.

DEV_OPS (21 files)

    1  scripts/promote-warehouse.ts                 DEV_OPS P0  → DELETED 2026-09-28 (hop rebuilt in Rust + SQL)
    2  scripts/promote-applemail.ts                 DEV_OPS P0  → PORT (shares 1)
    3  scripts/export-dev-projects-workspace.mjs    DEV_OPS P0  → PORT
    4  scripts/pull-prod-to-dev.mjs                 DEV_OPS P0  → RETIRE (branch reset is the path)
    5  scripts/mq-worker.ts                         DEV_OPS P0  → VERIFY, repoint to rust mq_runtime / RETIRE
    6  scripts/db-zombies.mjs                       DEV_OPS P1  → PORT
    7  scripts/import-property-photos.mjs           DEV_OPS P1  → PORT (one media importer)
    8  scripts/import-property-document.mjs         DEV_OPS P1  → PORT (same)
    9  scripts/import-missing-guide-images.mjs      DEV_OPS P1  → PORT (same)
    10 scripts/import-guide-images.mjs              DEV_OPS P1  → PORT (same)
    11 scripts/seed-flight-recorder-qa.ts           DEV_OPS P1  → PORT
    12 scripts/seed-projects-mvi2-dev.ts            DEV_OPS P1  → VERIFY → PORT P2 / RETIRE
    13 scripts/migration-ledger-baseline.mjs        DEV_OPS P2  → RETIRE
    14 scripts/fix-person-name-order-*.mjs          DEV_OPS P2  → RETIRE (applied)
    15 scripts/sprint.ts                            DEV_OPS P2  → RETIRE
    16 scripts/sprint-cleanup.ts                    DEV_OPS P2  → RETIRE
    17 scripts/workspace-cli.ts                     DEV_OPS P2  → RETIRE (worktrees forbidden)
    18 scripts/check-svar-widgets.mts               DEV_OPS P2  → RETIRE
    19 scripts/generate-form-review-pdf.ts          DEV_OPS P2  → PORT P2 / RETIRE
    20 scripts/rust-api-client.test.ts              DEV_OPS P2  → RETIRE
    21 scripts/rust-ui-mount.test.ts                DEV_OPS P2  → RETIRE

FORGE (27 files)

**SIX DEAD SUITES NO LONGER RUN (2026-09-28).** `pnpm test:harness` was
`node --import tsx --test scripts/*.test.ts` — a glob written when every file under `scripts/` could load.
Six of them (`forge-manifest`, `forge-packet-lint`, `forge-sync-agents`, `rust-api-client`, `rust-ui-mount`,
`whatsapp-coexistence-completion`) die at import on the deleted `legacy/`, so the harness was permanently red
for a reason nobody could fix, which is how a gate gets switched off. The runner is now
`scripts/test-harness.mjs`, which DERIVES its list instead of listing it: a suite carrying the
`⚠ BROKEN ON PURPOSE` banner is skipped and named, an unmarked one runs. A repaired file rejoins the run
automatically; a newly dead one is skipped the moment it is marked. Their Rust replacements are the fixtures
in `rust/forge/src/scope_manifest.rs` + `rust/cli/src/forge/manifest.rs` and the sibling modules below.

    22 scripts/forge-packet-lint.ts                 FORGE P0    → PORTED 2026-09-27 (rust/cli forge harness-lint)
    23 scripts/forge-packet-lint.test.ts            FORGE P0    → PORTED 2026-09-27 (39 fixtures in rust/cli/src/forge/*)
    24 scripts/forge-sync-agents.ts                 FORGE P0    → PORTED 2026-09-27 (rust/cli forge sync-agents)
    25 scripts/forge-sync-agents.test.ts            FORGE P0    → PORTED 2026-09-27 (fixtures in forge/sync_agents.rs)
    26 scripts/forge-manifest.ts                    FORGE P1    → PORTED 2026-09-28 (rust/cli forge manifest)
    27 scripts/forge-manifest.test.ts               FORGE P1    → PORTED 2026-09-28 (fixtures in forge/scope_manifest.rs + cli/forge/manifest.rs)
    28 scripts/forge-story-reset-config.ts          FORGE P1    → PORT (beside db_tool)
    29 scripts/forge-handoff.mjs                    FORGE P1    → VERIFY → RETIRE / PORT
    30 scripts/forge-human-gate-pass.mjs            FORGE P1    → VERIFY → PORT
    31 scripts/forge-test-stories.ts                FORGE P1    → PORT
    32 scripts/forge-record-stories.ts              FORGE P1    → PORT
    33 scripts/forge-ladder.ts                      FORGE P2    → PORT (lowest of the feeders)
    34 scripts/agent-work.ts                        FORGE P1    → RETIRE (verify forge_worker)
    35 scripts/agent-work-entry.ts                  FORGE P1    → RETIRE (forge_worker.rs replaces it)
    36 scripts/agent-runtime-invoke.ts              FORGE P1    → RETIRE
    37 scripts/agent-runtime-deepseek.ts            FORGE P1    → RETIRE
    38 scripts/forge-orchestrate-wake.ts            FORGE P1    → RETIRE
    39 scripts/forge-runtime-recover.ts             FORGE P1    → RETIRE
    40 scripts/forge-batch-release.mjs              FORGE P2    → RETIRE
    41 scripts/forge-holes-board.mjs                FORGE P2    → RETIRE
    42 scripts/update-core-daily.mjs                FORGE P2    → RETIRE
    43 scripts/update-core-daily-2.mjs              FORGE P2    → RETIRE
    44 scripts/update-core-daily-0910.mjs           FORGE P2    → RETIRE
    45 scripts/update-rel-intel-stories.mjs         FORGE P2    → RETIRE
    46 scripts/rel-intel-nav-close.mjs              FORGE P2    → RETIRE
    47 scripts/projects-workspace-board.mjs         FORGE P2    → RETIRE
    48 scripts/projects-workspace-scope-note.mjs    FORGE P2    → RETIRE

APP (29 files)

    49 scripts/apple-mailbox-intake.ts              APP P1      → VERIFY → PORT / promotion only
    50 scripts/apple-messages-intake.ts              APP P1      → VERIFY → PORT
    51 scripts/apple-messages-intake-proof.ts        APP P1      → fold into 50
    52 scripts/apple-messages-real-load.ts           APP P1      → fold into 50
    53 scripts/apple-calls-intake.ts                 APP P1      → PORT
    54 scripts/load-apple-contacts.ts                APP P1      → DONE 2026-09-28 (apple_contacts_load, SQL function)
    55 scripts/bank-transaction-load.ts              APP P1      → PORT
    56 scripts/gmail-metadata-sync.ts                APP P1      → VERIFY → PORT
    57 scripts/rel-intel-load-gmail.ts               APP P1      → VERIFY → PORT
    58 scripts/whatsapp-coexistence-completion.test.ts APP P1    → PORT as a Rust test
    59 scripts/calendar-eventkit-intake.ts           APP P2      → VERIFY → RETIRE
    60 scripts/eventkit-seam-proof.ts                APP P2      → VERIFY → PORT P2 (one seam proof)
    61 scripts/catchup-calendar-proof.ts             APP P2      → RETIRE
    62 scripts/catchup-calendar-attention-proof.ts   APP P2      → RETIRE
    63 scripts/catchup-dev-proof.ts                  APP P2      → RETIRE
    64 scripts/verify-crm-intake.mjs                 APP P2      → RETIRE / Rust integration test
    65 scripts/verify-crm-foundation.mjs             APP P2      → RETIRE
    66 scripts/verify-crm-email-intake.mjs           APP P2      → RETIRE
    67 scripts/verify-crm-communications-intake.mjs  APP P2      → RETIRE
    68 scripts/verify-crm-person-creation.mjs        APP P2      → RETIRE
    69 scripts/verify-website-intake.mjs             APP P2      → RETIRE
    70 scripts/verify-website-intake-general-enquiry.mjs APP P2 → RETIRE
    71 scripts/verify-needs-review-resolution.mjs    APP P2      → RETIRE
    72 scripts/core-daily-proof-12-13.ts             APP P2      → RETIRE
    73 scripts/rel-intel-proof-opps.ts               APP P2      → RETIRE
    74 scripts/rel-intel-proof-readmodel.ts          APP P2      → RETIRE
    75 scripts/verify-contract-mapper.ts             APP P2      → VERIFY
    76 scripts/verify-listing-client-fill.ts         APP P2      → VERIFY
    77 scripts/verify-listing-contract-bridge.ts     APP P2      → VERIFY

---

# Appendix B — added by the 2026-09-27 rigorous re-sweep (111 files)

The first sweep undercounted because it did not walk the whole tree. This appendix is generated from
the resolver, so every row is a verified fact of the same kind as Appendix A: the file **cannot load**
(value import of a module that does not exist, directly or transitively) or **cannot work** (it loads,
but a lazily-imported module it needs is gone). Verdicts are deliberately **PENDING**: the captain
must first say which of these capabilities is still wanted, because several are named by live
scheduled jobs (see "Live callers" above) and cannot be classified by a sweep.

      1 agent-runtime/accepted-candidate-publish.ts              CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '../lib/worker-workspace' (line 16))
      2 agent-runtime/agent-runtime-adapter.ts                   CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '../lib/server-error-capture' (line 24))
      3 agent-runtime/deepseek/deepseek-harness-adapter.ts       CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead '../agent-runtime-adapter' -> agent-runtime/agent-runtime-adapter.ts (line 23))
      4 agent-runtime/deterministic-assay-adapter.ts             CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead './agent-runtime-adapter' -> agent-runtime/agent-runtime-adapter.ts (line 10))
      5 agent-runtime/enqueue-lane.ts                            CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '@/legacy/workflow_app/forge/forge-lead-routing-prompt' (line 16))
      6 agent-runtime/execution-contract.ts                      CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '../lib/execution-target' (line 21))
      7 agent-runtime/factory.ts                                 CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead './registry' -> agent-runtime/registry.ts (line 11))
      8 agent-runtime/forge-topology.ts                          CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '@/legacy/workflow_app/definitions/forge-sdlc' (line 8))
      9 agent-runtime/gateway/cli-agent-adapter.ts               CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '../../lib/worker-workspace/provisioner' (line 9))
     10 agent-runtime/invoker.ts                                 CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead './registry' -> agent-runtime/registry.ts (line 12))
     11 agent-runtime/learn-loop.ts                              CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '../lib/worker-workspace/provisioner' (line 33))
     12 agent-runtime/opencode/opencode-harness-adapter.ts       CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead '../agent-runtime-adapter' -> agent-runtime/agent-runtime-adapter.ts (line 51))
     13 agent-runtime/orchestrate-apply.ts                       CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead './enqueue-lane' -> agent-runtime/enqueue-lane.ts (line 8))
     14 agent-runtime/orchestrate.ts                             CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead './enqueue-lane' -> agent-runtime/enqueue-lane.ts (line 12))
     15 agent-runtime/registry.ts                                CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead './agent-runtime-adapter' -> agent-runtime/agent-runtime-adapter.ts (line 17))
     16 agent-runtime/repo-context.ts                            CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '../lib/forge-decision' (line 14))
     17 agent-runtime/repositories.ts                            CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 12))
     18 agent-runtime/run-guardrails.ts                          CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead './agent-runtime-adapter' -> agent-runtime/agent-runtime-adapter.ts (line 18))
     19 agent-runtime/tunit-adapter.ts                           CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: imports dead './agent-runtime-adapter' -> agent-runtime/agent-runtime-adapter.ts (line 21))
     20 agent-runtime/write-policy.ts                            CANNOT LOAD FORGE (agent plane)        → PENDING (captain classifies; reason: missing module '../lib/worker-workspace/provisioner' (line 9))
     21 scripts/apple-gateway-worker.ts                          CANNOT LOAD APP (channel pipeline)     → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 14))
     22 scripts/apple-mail-envelope-intake.ts                    CANNOT LOAD APP (channel pipeline)     → PENDING (captain classifies; reason: imports dead './lib/pool-executor' -> scripts/lib/pool-executor.ts (line 41))
     23 scripts/apple-reminders-intake.ts                        CANNOT LOAD APP (channel pipeline)     → PENDING (captain classifies; reason: missing module '@/legacy/db/reminder-landing' (line 10))
     24 scripts/audit-phone-identities.ts                        CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 8))
     25 scripts/backfill-cost-widgets.ts                         CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 19))
     26 scripts/capture-driver-value-formats.ts                  CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 26))
     27 scripts/cleanse-dev-fixtures.ts                          CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/fixture-cleanup' (line 21))
     28 scripts/cleanup-apple-rows.ts                            CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: imports dead './lib/pool-executor' -> scripts/lib/pool-executor.ts (line 8))
     29 scripts/column-writer-audit.ts                           CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 33))
     30 scripts/core-daily-proof-01.ts                           CANNOT LOAD RETIRE (proof-of-a-past-run) → PENDING (captain classifies; reason: missing module '@/legacy/db/follow-up' (line 13))
     31 scripts/core-daily-proof-03-04.ts                        CANNOT LOAD RETIRE (proof-of-a-past-run) → PENDING (captain classifies; reason: missing module '@/legacy/db/follow-up' (line 13))
     32 scripts/core-daily-proof-06-11.ts                        CANNOT LOAD RETIRE (proof-of-a-past-run) → PENDING (captain classifies; reason: missing module '@/legacy/db/follow-up' (line 13))
     33 scripts/core-daily-proof-07-08.ts                        CANNOT LOAD RETIRE (proof-of-a-past-run) → PENDING (captain classifies; reason: missing module '@/legacy/db/follow-up' (line 13))
     34 scripts/core-daily-proof-09-10.ts                        CANNOT LOAD RETIRE (proof-of-a-past-run) → PENDING (captain classifies; reason: missing module '@/legacy/db/follow-up' (line 14))
     35 scripts/core-daily-proof-15.ts                           CANNOT LOAD RETIRE (proof-of-a-past-run) → PENDING (captain classifies; reason: missing module '@/legacy/db/follow-up' (line 13))
     36 scripts/create-deep1-story.ts                            CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 12))
     37 scripts/exec-batch1-story.ts                             CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 14))
     38 scripts/fix-person-name-order-julio-pimentel-ortiz.mjs   CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '../legacy/db/forge-db.ts' (line 34))
     39 scripts/forge-batch-status.ts                            CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 15))
     40 scripts/forge-board-sync.ts                              CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 26))
     41 scripts/forge-cleanup-dev.ts                             CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 19))
     42 scripts/forge-consistency.ts                             CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-workflow-evidence' (line 11))
     43 scripts/forge-decision.ts                                CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-decision' (line 30))
     44 scripts/forge-doctor.ts                                  CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 34))
     45 scripts/forge-learn.ts                                   CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/lib/execution-target' (line 24))
     46 scripts/forge-read-tools.ts                              CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 25))
     47 scripts/forge-resume-door.ts                             CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/workflow_app/forge/forge-hold-resolve' (line 9))
     48 scripts/forge-roi.ts                                     CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-roi' (line 22))
     49 scripts/forge-scorecard.ts                               CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/workflow_app/forge/forge-scorecard' (line 10))
     50 scripts/forge-static-gate.ts                             CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/workflow_app/forge/forge-static-gate' (line 11))
     51 scripts/forge-story-reset.ts                             CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 25))
     52 scripts/forge-tools.ts                                   CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/workflow_app/forge/forge-tool-catalog' (line 26))
     53 scripts/forge-triage.ts                                  CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/workflow_app/forge/failure-classifier' (line 9))
     54 scripts/import-l-regrid.ts                               CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 32))
     55 scripts/inspect-dev-identity.ts                          CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 10))
     56 scripts/lib/pool-executor.ts                             CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 15))
     57 scripts/normalize-story-priorities.ts                    CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: imports dead './lib/pool-executor' -> scripts/lib/pool-executor.ts (line 11))
     58 scripts/probe-agent-work-state.ts                        CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 10))
     59 scripts/probe-batch-schedule.ts                          CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 10))
     60 scripts/probe-batch-sync.ts                              CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 10))
     61 scripts/probe-cockpit-data-state.ts                      CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 10))
     62 scripts/probe-engine-withdraw.ts                         CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 10))
     63 scripts/probe-error-capture.ts                           CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/app-error' (line 14))
     64 scripts/probe-flight-recorder-timing.ts                  CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/workflow_app/engine-client' (line 12))
     65 scripts/probe-flight-recorder.ts                         CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/database-gateway' (line 13))
     66 scripts/probe-forge-observer.ts                          CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 34))
     67 scripts/probe-handoff-path.ts                            CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 10))
     68 scripts/probe-kind-routing.ts                            CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 12))
     69 scripts/probe-learn-dedupe.ts                            CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/agent-work' (line 12))
     70 scripts/probe-move-write.ts                              CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 10))
     71 scripts/probe-sorter-moves.ts                            CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/lib/story-moves' (line 10))
     72 scripts/project-apple-contacts.ts                        CANNOT LOAD APP P1                     → DONE 2026-09-28 (apple_contacts_project, SQL function)
     73 scripts/promote-relationship-evidence.ts                 CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 24))
     74 scripts/provision-catchup-task-fixtures.ts               CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 21))
     75 scripts/provision-dev-google-identity.ts                 CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 13))
     76 scripts/provision-dev-root.ts                            CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 13))
     77 scripts/provision-v1-roles.ts                            CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 10))
     78 scripts/regrid-csv-load.ts                               CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/regrid-culebra-parcel' (line 12))
     79 scripts/regrid-property-lookup.ts                        CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/app-error' (line 22))
     80 scripts/seed-accounting-fixture.ts                       CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 18))
     81 scripts/seed-forge-sdlc.ts                               CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 20))
     82 scripts/seed-issue-fixtures.ts                           CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 17))
     83 scripts/seed-projects-test.ts                            CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 12))
     84 scripts/seed-split-dogfood-story.ts                      CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 13))
     85 scripts/set-story-status.ts                              CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 29))
     86 scripts/story-preflight.ts                               CANNOT LOAD FORGE                      → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 26))
     87 scripts/sync-arch-handoff.ts                             CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: imports dead './lib/pool-executor' -> scripts/lib/pool-executor.ts (line 26))
     88 scripts/sync-forge-history.ts                            CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 20))
     89 scripts/update-auth08-story.ts                           CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 9))
     90 scripts/update-harden05-story.ts                         CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 9))
     91 scripts/update-mac-sync-cal-story.ts                     CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 9))
     92 scripts/update-ops11a-story.ts                           CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 9))
     93 scripts/update-projects-anchor-story.ts                  CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 15))
     94 scripts/update-projects-anchor2-story.ts                 CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 9))
     95 scripts/update-reference-stories.ts                      CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/forge-db' (line 11))
     96 scripts/update-security-stories.ts                       CANNOT LOAD UNCLASSIFIED               → PENDING (captain classifies; reason: missing module '@/legacy/db/storyboard' (line 10))
     97 scripts/verify-contract-service.ts                       CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/services/composition' (line 13))
     98 scripts/verify-dev-google-provision.ts                   CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 10))
     99 scripts/verify-dev-root.ts                               CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 10))
    100 scripts/verify-forms-service.ts                          CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/services/composition' (line 14))
    101 scripts/verify-issues.ts                                 CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/db/issues' (line 13))
    102 scripts/verify-l-regrid.ts                               CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 29))
    103 scripts/verify-property-publication.ts                   CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/db/client' (line 23))
    104 scripts/verify-public-reads-service.ts                   CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/services/composition' (line 14))
    105 scripts/verify-vault-service.ts                          CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/services/composition' (line 13))
    106 scripts/verify-wbs-service.ts                            CANNOT LOAD APP/QA probe               → PENDING (captain classifies; reason: missing module '@/legacy/services/composition' (line 13))
    107 scripts/forge-tree-residue-sweep.ts                      CANNOT WORK FORGE                      → PENDING (captain classifies; reason: dynamic import '@/legacy/db/client' is gone)
    108 scripts/normalize-phone-identities.ts                    CANNOT WORK UNCLASSIFIED               → PENDING (captain classifies; reason: dynamic import '@/legacy/db/client' is gone)
    109 scripts/seed-jessica-project.ts                          CANNOT WORK UNCLASSIFIED               → PENDING (captain classifies; reason: dynamic import '@/legacy/services/core' is gone)
    110 scripts/test-story.ts                                    CANNOT WORK FORGE                      → PENDING (captain classifies; reason: dynamic import '@/legacy/db/client' is gone)
    111 scripts/workflow-cli.ts                                  CANNOT WORK UNCLASSIFIED               → PENDING (captain classifies; reason: dynamic import '@/legacy/workflow_app/application-port' is gone)
