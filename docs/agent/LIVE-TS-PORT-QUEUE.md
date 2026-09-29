# Live TypeScript — the port queue, and who to ask before touching it

> **2026-09-29, later the same day — the gate named below was removed on the owner's order.** Two ratchets had landed
> for one fact (`pnpm ts:count` here, and `scripts/ts-ratchet.sh` + `docs/agent/ts-allowlist.txt` for the whole
> non-legacy estate). The owner kept the wider one: **`docs/agent/ts-allowlist.txt` is the single writer — 259 tracked
> TS/JS files at the handover, and it may only shrink.** `scripts/live-ts-gate.sh` and `docs/agent/live-ts-baseline.txt`
> were deleted in the same commit, together with the `ts:count` commands and the CI step that ran them. This file is
> kept as the record of the sweep below; its old classification is gone, and **every count in it is historical** —
> `docs/agent/ts-allowlist.txt` is the only current fact.

The gate was `pnpm ts:count` (`scripts/live-ts-gate.sh`, baseline `docs/agent/live-ts-baseline.txt`, CI step
`live-TS count (may only fall)` in the `static gates` job). It counted **69** loadable files, and it is gone too.

This file used to be the other half of `docs/agent/BROKEN-TS-INVENTORY.md`: that inventory listed files that
**cannot load**, this one listed files that **load and run**, and the split is why a green `broken:ts:sweep` came to
be read as "the TypeScript is retired" while 69 working files sat outside `legacy/`. That split no longer decides
anything. **`docs/agent/ts-allowlist.txt` is the single writer of what remains, and it may only shrink.**

## 2026-09-29 — the sweep the owner ordered, and what is left in this half

The order: for every file in the allowlist, **delete it if it cannot load or nothing calls it**; if it is live and
used, **port it to Rust or a shell script and then delete it**; remove any `pnpm` command or CI step that pointed at
it; remove its allowlist row in the same commit. Two lanes ran it: **Cline — everything outside `scripts/`, plus
`scripts/a*` to `scripts/f*`**; GPT — `scripts/g*` to `scripts/z*`.

This lane is done. **139 entries in this half, 4 left**, in three sweep commits and two port commits:

1. `08648e86` — 88 files, every one carrying the `⚠ BROKEN ON PURPOSE` banner (the sweep's two unloadable
   categories — `cannot load`, and `loads, but lazy target gone`, `scripts/broken-ts-sweep.mjs:136-139`), all with
   **zero live importers**; 17 `pnpm` commands that pointed only at them; and the `eslint-suppressions.json` entries
   that went stale with them (275 file entries before the sweep, 7 now — a suppression for a file that no longer
   exists is itself a lint failure).
2. `7acb3d35` — 43 files that do load but that **no live root reaches**, decided by a reverse-import walk from every
   `pnpm` command, CI step and shell script (a dead file cannot be imported by a live one): 29 `agent-runtime/`
   modules whose only importers were the files deleted in step 1; the 4-file `workflow_engine/lib/workflow` kernel
   and 2 `testv2` fixtures (`rust/core/workflow` replaces them, and `rust/FORGE_CUTOVER.md` already said delete once
   the Rust host is default); `forge-engine-worker.ts` (a shim whose body is `cargo run -p forge --bin forge`);
   `forge-silent-failure-gate.ts` + `agent-runtime/silent-failure-patterns.ts`; `app-runtime-boundary.mjs` + both
   `.dependency-cruiser` configs; `check-trailing-whitespace.ts`; `agent-scheduler.test.mjs`; `postcss.config.mjs`.
3. `2580d44e` — `check-svar-widgets.mts`, the last bannered file in this half (it imported the deleted
   `ui/projects/*`), with `check:widgets`; plus the dead-command `BASELINE` re-measured on the smaller menu
   (53 → 25 → 19).
4. *(port, not sweep)* **the two ledger gates are Rust now**: `forge ts-sweep`
   (`rust/cli/src/forge/ts_sweep.rs`) and `forge dead-commands` (`rust/cli/src/forge/dead_commands.rs`),
   with the `package.json` names kept (`pnpm broken:ts:sweep`, `pnpm broken:ts:commands`), both scripts
   deleted, the CI step moved into the `rust` job (it needs cargo, and the `static` job has none), and
   `BASELINE` re-measured on the tree this landed on: **19 → 10 → 0** — the menu names no bannered file at
   all, out of 97 scripts. Both gates were run against the TypeScript they replace and agree line for line,
   including the `--check` refusal and `--format json`.
5. *(port, not sweep)* **`scripts/agent-scheduler.mjs` is Rust**: `rust/cli/src/launchd/`
   (`cargo run -p cli -- launchd agent-worker <render|install|status|run|stop|uninstall>`), the five
   `pnpm agent:scheduler:*` names repointed, the script deleted, its `ts-allowlist.txt` row and its
   `eslint-suppressions.json` entry removed with it. Proven by **byte-diff, not by reading**: Rust `render`
   was identical to the TypeScript's render AND to the plist actually installed on this machine, and `status`
   and the `run` refusal (exit 2) were diffed line for line against the TypeScript before it was deleted.
   The other two installers reuse this module; see §6 of the handoff.

Two of those were decided by fact, not by guessing: `postcss.config.mjs` went only after the Tailwind v4 CLI
built `rust/ui/styles/app.css` byte-identically without it (its consumer was Next.js; `scripts/site-build.sh` runs
`npx tailwindcss` directly), and `scripts/tmp-go.sh` was repointed from the deleted shim to `pnpm forge:engine`.

Two false positives the gates themselves produced were fixed in the gates' own terms: deleting a file another lane's
`scripts/oc-probe3.ts` lazy-imported made that file unloadable and unmarked, which `broken:ts:sweep` correctly refuses
as DRIFT (it was already ruled DELETE in `docs/agent/TS-TRIAGE.md:183`, so the verdict was applied); and two learn packets
that cited deleted files had their citations **removed, not baselined**, because
`docs/agent/harness-lint-baseline.json` states in its own first line that the list may only shrink.

### Kept in this half, with the reason and the port each one needs (7)

| File | Why it is still here | The port it needs |
| --- | --- | --- |
| `eslint.config.mjs` | `pnpm lint` is a CI gate and eslint is configured in JS; delete it and the lint stops | none exists — it goes when the last JS tool does |
| ~~`scripts/broken-ts-sweep.mjs`~~ | **PORTED 2026-09-29** → `rust/cli/src/forge/ts_sweep.rs` (`cargo run -p cli -- forge ts-sweep`); its output was verified byte-identical, and one thing changed on purpose: the drift lists print sorted, because the filesystem's `readdir` order is not stable enough to diff | — |
| ~~`scripts/dead-command-sweep.mjs`~~ | **PORTED 2026-09-29** → `rust/cli/src/forge/dead_commands.rs` (`forge dead-commands`); text, `--format json` and the `--check` refusal are byte-identical, `BASELINE` is re-measured — the menu is read through an `IndexMap` so the row order stays `package.json`'s | — |
| `scripts/agent-scheduler.mjs` | **PORTED 2026-09-29** → `rust/cli/src/launchd/` (`forge`'s sibling: `launchd agent-worker`), proven by byte-diff of `render` against the installed plist and line-for-line diffs of `status` and the `run` refusal | — (done) |
| `scripts/apple-sync-agent.mjs` | same, for the installed `com.culebraluxe.apple-sync` agent (6 `pnpm` commands) | as above |
| `scripts/calendar-sync-agent.mjs` | same, for the installed `com.culebraluxe.calendar-sync` agent (5 `pnpm` commands) | as above |
| `scripts/apple-local-listener.mjs` | the Apple intake listener, **loaded and running under launchd**; a long-lived process, not a utility | a Rust listener, or accept it as part of the macOS bridge beside `apple-messages-export/` |

One landmine, named: `scripts/agent-scheduler.test.mjs` was deleted although nothing ran it — its subject (the
launchd installer above) is still unported, so that coverage is owed back as a Rust test.

`e2e/portal-nav-smoke.mjs` is not in the allowlist by design: `scripts/ts-ratchet.sh:18` skips `e2e/*` (the WebKit
exception). It drives the **current** Yew UI and is named by `pnpm debug:portal-nav`.

The other lane (`scripts/g*`–`scripts/z*`) was still in flight when this was written. **The allowlist is the
authority for what remains**, never a count in prose — including the one in this file.
