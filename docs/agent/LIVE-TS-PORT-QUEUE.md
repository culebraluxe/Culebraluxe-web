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

This lane is done. **139 entries in this half, 7 left**, in three commits:

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
| `scripts/broken-ts-sweep.mjs` | the ledger gate CI runs (`pnpm broken:ts:sweep`); it measures this retirement, so it must outlive the last file it measures | `rust/cli` `forge ts-sweep` — the banner rule, the import resolution, the counts |
| `scripts/dead-command-sweep.mjs` | the `pnpm` menu ledger; `--check` refuses a count that moves without the baseline | same subcommand family |
| `scripts/agent-scheduler.mjs` | installs/status/run/stop/uninstall `com.culebraluxe.agent-worker`, whose plist is **installed on this machine** and drives `pnpm agent:work` (already Rust) | a `rust/cli` launchd subcommand — nothing under `rust/` mentions `LaunchAgents` today |
| `scripts/apple-sync-agent.mjs` | same, for the installed `com.culebraluxe.apple-sync` agent (6 `pnpm` commands) | as above |
| `scripts/calendar-sync-agent.mjs` | same, for the installed `com.culebraluxe.calendar-sync` agent (5 `pnpm` commands) | as above |
| `scripts/apple-local-listener.mjs` | the Apple intake listener, **loaded and running under launchd**; a long-lived process, not a utility | a Rust listener, or accept it as part of the macOS bridge beside `apple-messages-export/` |

One landmine, named: `scripts/agent-scheduler.test.mjs` was deleted although nothing ran it — its subject (the
launchd installer above) is still unported, so that coverage is owed back as a Rust test.

`e2e/portal-nav-smoke.mjs` is not in the allowlist by design: `scripts/ts-ratchet.sh:18` skips `e2e/*` (the WebKit
exception). It drives the **current** Yew UI and is named by `pnpm debug:portal-nav`.

The other lane (`scripts/g*`–`scripts/z*`) was still in flight when this was written. **The allowlist is the
authority for what remains**, never a count in prose — including the one in this file.
