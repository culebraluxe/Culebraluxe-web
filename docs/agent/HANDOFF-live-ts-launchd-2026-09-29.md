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
| S3 | `scripts/agent-scheduler.mjs` (303 lines) manages the **installed** `com.culebraluxe.agent-worker` LaunchAgent, cadence 180s, wrapper `scripts/agent-worker-once.sh` | `ls ~/Library/LaunchAgents \| grep culebra` → 4 plists; `launchctl print gui/$(id -u)/com.culebraluxe.agent-worker` |
| S4 | `scripts/apple-sync-agent.mjs` (315 lines) manages `com.culebraluxe.apple-sync`, 08:00/18:00 local, invoking a Swift FDA launcher that execs a deployed `apple-sync.sh` | same, label `com.culebraluxe.apple-sync` |
| S5 | `scripts/calendar-sync-agent.mjs` (295 lines) manages `com.culebraluxe.calendar-sync`, cadence `CALENDAR_SYNC_CADENCE_SECONDS` (default 1800), snapshot `/tmp/culebraluxe-calendar.json` | same, label `com.culebraluxe.calendar-sync` |
| S6 | `scripts/apple-local-listener.mjs` is a live loopback HTTP server under the installed `com.culebraluxe.apple-local-listener.plist` | `launchctl print gui/$(id -u)/com.culebraluxe.apple-local-listener`; the plist names the port and the JS file |
| S7 | Each installer renders its plist from a tracked template by **plain placeholder substitution** — no per-agent XML lives in the code | `for f in scripts/*.plist.template; do grep -o '{{[A-Z_]*}}' "$f" \| sort -u; done` |
| S8 | `scripts/eslint.config.mjs` is grandfathered by the owner and is **not** to be ported | the captain's word, 2026-09-29 |

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

## 5. NOT VERIFIED — the honest gaps

- No Rust launchd code exists yet: `git grep -n 'LaunchAgents\|launchctl' -- rust/` returns nothing.
- The three installers have **never been run by this lane**; their `install`/`uninstall` paths are read, not
  exercised. `status`/`run` on the captain's machine are the only safe live checks.
- `scripts/agent-scheduler.test.mjs` was deleted with the earlier sweep, so the installer has **no test** anywhere:
  the Rust port owes one back, which is why §6.1 is a byte-diff rather than a claim.
- The listener's port number, origin allowlist and spawn behaviour were read from the file, never exercised live.

## 6. OPEN — the next actions, in order

1. **Port `agent-scheduler.mjs` + `apple-sync-agent.mjs` + `calendar-sync-agent.mjs` as ONE `rust/cli` module.**
   Shared: `machine_paths(env)` (`HOME`, `CULEBRALUXE_LAUNCHAGENTS_DIR`, `CULEBRALUXE_SUPPORT_DIR`, the three
   per-agent log-dir variables, `uid`, `gui/<uid>`, `gui/<uid>/<LABEL>`), `escape_xml` (`< > & ' "` → the five
   entities, in the order the JS uses), `render_plist(template, substitutions)` (literal replace-all per
   placeholder), `launchctl(args)` (`/bin/launchctl`, capturing stdout/stderr), `plutil -lint`, `sha256_file`,
   `short_sha` (12 chars), and the five verbs `install | status | run | stop | uninstall` (+ `verify-tcc` for
   apple-sync). Per agent only the descriptor differs: label, template name, cadence, log dir, deployed artifacts,
   the env `run` passes, and the log-tail rules.
   **Acceptance, before deleting anything:** add a `render` verb, then byte-diff the Rust render against the
   **installed** plist for each label —
   `diff <(./rust/target/debug/cli launchd apple-sync render) ~/Library/LaunchAgents/com.culebraluxe.apple-sync.plist`
   — and the same for `agent-worker` and `calendar-sync`. A byte-identical render is the proof; nothing else is.
   Then repoint the 16 `package.json` names (`agent:scheduler:*` 5, `apple:sync:*` 6, `calendar:sync:*` 5), delete
   the three `.mjs`, and drop their three `docs/agent/ts-allowlist.txt` rows in the same commit.
2. **Only after §6.1 is green:** add the owed test — a plist-render unit test per agent against a fixture, plus the
   `wrapperIntegrity` mismatch rule (`run` refuses when the deployed wrapper differs, exit 2).
3. **Then the listener**, as its own story: `scripts/apple-local-listener.mjs` is a long-lived server on loopback
   with an Origin allowlist that spawns the Apple launcher. It is the one file here that is a **new process** rather
   than a new CLI verb: it needs a Rust binary, a new/edited plist, `launchctl` re-bootstrap and a live
   browser-to-loopback check. Do not fold it into §6.1.

## 7. ASK THE OWNER

- May this lane run `agent:scheduler:install` / `apple:sync:install` / `calendar:sync:install` once the ports are
  written, to prove the Rust installer bootstraps the same job the Node one did? **Yes** → the port is verified end
  to end by a live re-install with the byte-identical plist; **No** → the proof stops at the plist byte-diff plus a
  read-only `status`, and §5 keeps saying so.
