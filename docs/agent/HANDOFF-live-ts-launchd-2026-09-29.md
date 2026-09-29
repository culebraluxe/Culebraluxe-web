# Handoff — the live-TypeScript lane: the launchd installers and the intake listener (2026-09-29)

Face value: the dead-TS sweep is **finished for this lane's half** (`docs/agent/LIVE-TS-PORT-QUEUE.md`: 139
allowlist entries → 5), and the two ledger gates that measured it are **ported to Rust and pushed** (§4). What is
left is the macOS bridge: three launchd installers and one long-lived listener. They are **not** dead files — they
are installed and running on this machine (§1 S3–S6) — so they need a port, not a delete.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | The ledger gates are Rust: `forge ts-sweep` (`rust/cli/src/forge/ts_sweep.rs`), `forge dead-commands` (`rust/cli/src/forge/dead_commands.rs`); `pnpm broken:ts:sweep` / `:commands` are repointed to them | `pnpm broken:ts:sweep` → `10 scanned / 0 cannot load / 0 cannot work / 0 marked`, "the tree and the inventory agree" |
| S2 | The menu is clean: **0** dead commands of 97 scripts, and that baseline is enforced by CI | `pnpm broken:ts:commands --check` → `ok: 0 dead commands, at the baseline.` |
| S3 | **`launchd agent-worker` (Rust) manages the installed `com.culebraluxe.agent-worker` LaunchAgent** — cadence 180s, wrapper `scripts/agent-worker-once.sh`. `scripts/agent-scheduler.mjs` no longer exists | `cargo run -p cli -- launchd agent-worker status`; `pnpm agent:scheduler:status` |
| S4 | `scripts/apple-sync-agent.mjs` (315 lines) manages `com.culebraluxe.apple-sync`, 08:00/18:00 local, invoking a Swift FDA launcher that execs a deployed `apple-sync.sh` | same, label `com.culebraluxe.apple-sync` |
| S5 | `scripts/calendar-sync-agent.mjs` (295 lines) manages `com.culebraluxe.calendar-sync`, cadence `CALENDAR_SYNC_CADENCE_SECONDS` (default 1800), snapshot `/tmp/culebraluxe-calendar.json` | same, label `com.culebraluxe.calendar-sync` |
| S6 | `scripts/apple-local-listener.mjs` is a live loopback HTTP server under the installed `com.culebraluxe.apple-local-listener.plist` | `launchctl print gui/$(id -u)/com.culebraluxe.apple-local-listener`; the plist names the port and the JS file |
| S7 | Each installer renders its plist from a tracked template by **plain placeholder substitution** — no per-agent XML lives in the code | `for f in scripts/*.plist.template; do grep -o '{{[A-Z_]*}}' "$f" \| sort -u; done` |
| S8 | `scripts/eslint.config.mjs` is grandfathered by the owner and is **not** to be ported | the captain's word, 2026-09-29 |
| S9 | **The scheduled Forge worker is failing right now, every three minutes** — `wrapper: MISMATCH repo=b2b68631e932 deployed=54ee17d54a3b`, last invocations `pass=1 end exit=1` / `stop: Forge returned non-zero exit=1`. The deployed wrapper has drifted from the repository's. **Not caused by this port** | `cargo run -p cli -- launchd agent-worker status` (read-only) |

Placeholders per template, measured: `agent-worker` → `{{CADENCE_SECONDS}} {{HOME}} {{LABEL}} {{LOG_DIR}}
{{REPO_ROOT}} {{SUPPORT_DIR}}`; `apple-sync` → those plus `{{LAUNCHER}} {{PATH}} {{SCRIPT}}`; `calendar-sync` → the
first six plus `{{APPLE_LAUNCHER}} {{SNAPSHOT}}`.

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | Running `install` for any of the three agents | the Captain | Rendering and `status` are read-only and free; **bootstrapping a job that syncs PROD twice daily is his call**. Ask before `install`. |
| H2 | `pnpm forge:clean` / anything on `DATABASE_URL_PROD` | the Captain | This lane needs neither. Do not reach for them. |
| H3 | The other lane (`scripts/g*`–`scripts/z*`) | the other agent, by name | Do not edit its files or its allowlist rows. |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Port the three installers | this file §6, then the three `.mjs` files' `machinePaths` / `renderPlist` / `install` / `printStatus` | `rust/cli/src/launchd/*.rs` (new), `rust/cli/src/forge/mod.rs`, `rust/cli/src/main.rs`, `package.json` |
| Port the listener | this file §6.3 | a Rust binary + a new plist, `launchctl` re-bootstrap |
| Prove a port faithful | §6.1–6.2 | the installed plists, byte-diffed against the Rust render |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `2e5fd282` | the two ledger gates → Rust; both `.mjs` deleted; `package.json` repointed; CI step moved to the `rust` job; `BASELINE` re-measured to 0; 5 docs updated | `cargo test -p cli` 101 passed (13 new); `pnpm broken:ts:sweep`, `pnpm broken:ts:commands --check`, `scripts/ts-ratchet.sh`, `pnpm test:harness`, `pnpm lint`, `pnpm forge:packet-lint` (0 failures), `git diff --check` — all green, and both ports were diffed **byte-identical** against the TypeScript before it was deleted |
| `28052a4c` | `scripts/agent-scheduler.mjs` → `rust/cli/src/launchd/` (`launchd agent-worker`), 5 `pnpm` names repointed, script + allowlist row + eslint suppression deleted, template comment corrected | `cargo test -p cli` 110 passed (9 new); `render` Byte-diffed against the TypeScript's render **and** against the installed plist; `status` and the `run` refusal (exit 2) diffed line for line; `ts-ratchet` PASS 10; `broken:ts:sweep` 0/0/0/0; `broken:ts:commands --check` ok at 0; `pnpm lint`, `test:harness`, `forge:packet-lint` green |

## 5. NOT VERIFIED — the honest gaps

- `launchd agent-worker install`, `stop` and `uninstall` have **never been executed** — not by the TypeScript
  this port replaced, and not by the Rust. They are read + reviewed only; the plist they would write is proved by
  the byte-diff, the `launchctl` calls around it are not. H1 is why.
- After the template's comment correction, `render` differs from the installed plist by **those two comment lines
  only** (`agent-scheduler.mjs` → `launchd/agent_worker.rs`, and 5 → 3 minutes in the cadence prose). A re-install
  regenerates them; until then the installed plist's comment names a file that no longer exists.
- The two remaining installers (`apple-sync`, `calendar-sync`) were read, not ported; their `install` paths do
  more than the worker's (swift build, codesign, PATH detection, log tails) and each needs its own descriptor.
- `scripts/agent-scheduler.test.mjs` was deleted with the earlier sweep, so the installer has **no behavioural
  test** anywhere. What exists now: 9 Rust tests over the render, the integrity rule, the invocation tail and the
  launchctl `disabled` parsing — not an installer test.
- The listener's port number, origin allowlist and spawn behaviour were read from the file, never exercised live.

## 6. OPEN — the next actions, in order

1. **Port `apple-sync-agent.mjs` and `calendar-sync-agent.mjs` onto the module that now exists.** Read
   `rust/cli/src/launchd/{mod.rs,agent_worker.rs}` first: `Machine`, `escape_xml`, `render_plist`, `launchctl`,
   `plutil_lint`, `sha256_file`, `short_sha`, `write_with_mode` are all shared already. Per agent only the
   descriptor differs — label, template, cadence (apple-sync has none: 08:00/18:00 — and calendar-sync's
   `CALENDAR_SYNC_CADENCE_SECONDS` defaults to 1800), log dir, deployed wrapper/launcher, the `{{PATH}}` and
   `{{SNAPSHOT}}` substitutions apple-sync and calendar-sync add, and their extra verbs (`verify-tcc`) and log
   tails. **Acceptance before deleting either file:** `render` byte-identical to the TypeScript's render (the
   harness that proves it is 10 lines of `node --input-type=module -e` importing `renderPlist` from the file), and
   `status` diffed line for line. Then repoint the 11 `pnpm` names and drop the two allowlist rows.
2. **Then the listener**, as its own story: `scripts/apple-local-listener.mjs` is a long-lived server on loopback
   with an Origin allowlist that spawns the Apple launcher. It is the one file here that is a **new process** rather
   than a new CLI verb: it needs a Rust binary, a new/edited plist, `launchctl` re-bootstrap and a live
   browser-to-loopback check. Do not fold it into §6.1.
3. **The `install` verbs still owe a live proof** (§5). If the Captain grants H1, one re-install per agent closes it:
   the plist is byte-identical by construction, so the thing being proved is the `launchctl` sequence around it.

## 7. ASK THE OWNER

- May this lane run `agent:scheduler:install` / `apple:sync:install` / `calendar:sync:install` once the ports are
  written, to prove the Rust installer bootstraps the same job the Node one did? **Yes** → the port is verified end
  to end by a live re-install with the byte-identical plist; **No** → the proof stops at the plist byte-diff plus a
  read-only `status`, and §5 keeps saying so.
