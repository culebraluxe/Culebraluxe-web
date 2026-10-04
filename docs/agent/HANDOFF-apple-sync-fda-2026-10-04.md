# Handoff — the Apple sync was dead on a revoked privacy grant, not a path (2026-10-04)

Written from `lane-deep` while run 4 of `com.culebraluxe.apple-sync` was still executing (the export had landed and the
PROD intake was mid-pass). Everything in §1 is true regardless of how the intake ends; §4 says which half is verified.

## 1. STATUS — what is true right now

| # | Fact | How to check it |
| --- | --- | --- |
| S1 | `com.culebraluxe.apple-sync` failed **every** run from 2026-10-03 13:22 until 05:39 on 10-04: `last exit code 1`, tail `ERROR: cannot open Messages DB read-only: authorization denied`, `status=FAILED duration_s=3`. It is TCC, not a path — the exporter dies before the intake, so no data was ever half-loaded. | `pnpm apple:sync:status`; `/Users/Shared/dev/build/logs/apple-sync.log` |
| S2 | The cause is a code-hash change: the deployed launcher was rebuilt and ad-hoc re-signed at 13:22 (`CDHash=52caf24b…`, `flags=adhoc`, `TeamIdentifier=not set`), which stopped matching the existing Full Disk Access row. | `codesign -dv --verbose=4 ".../Application Support/CulebraLuxe/apple-sync-launcher"` |
| S3 | The Captain re-granted FDA at ~05:39 on 10-04 and it **took**: a throwaway LaunchAgent running the deployed launcher `--verify-tcc` returned `TCC VERIFY: OK chat.db opened READ-ONLY message_count=95996`, exit 0. | recipe in `docs/agent/MEMORY.md`, entry 2026-10-04, fact (2) |
| S4 | The path half was **already correct** — do not re-diagnose it. Deployed wrapper is byte-identical to `scripts/apple-sync.sh`; plist and the *loaded* job both carry `CULEBRALUXE_REPO=/Users/Shared/dev/src/Culebraluxe-web` and `CULEBRALUXE_APPLE_LOG_DIR=/Users/Shared/dev/build/logs`; zero `Documents/Culebraluxe-web` hits in loaded config. | `launchctl print gui/501/com.culebraluxe.apple-sync \| sed -n '/environment = {/,/}/p'` |
| S5 | The one dead string left is inert: the launcher's own fallback default `~/Documents/Culebraluxe-web` at `apple-messages-export/Sources/apple-sync-launcher/main.swift:33`, overridden by the plist env. Only a Swift rebuild changes it — and that rebuild costs the FDA grant. | `strings ".../apple-sync-launcher" \| grep Documents` |
| S6 | Run 4 was driven with `launchctl kickstart gui/501/com.culebraluxe.apple-sync` (the Captain's "run it"), not by spawning the launcher from a shell — same launchd/TCC context, same lock, launchd-visible exit code. Export landed fresh: `exported handles=2616 messages=95996 min=2023-10-27T16:42:53.000Z max=2026-10-04T01:03:46.138Z`, `validation OK`, `messages.jsonl` 66,795,835 bytes at 05:44:49, `manifest.json` 05:44:55 — first fresh export since 2026-09-28. | `tail -20 /Users/Shared/dev/build/logs/apple-sync.log`; `ls -laT /Users/Shared/dev/src/Culebraluxe-web/public/upload/data/apple-messages-export/` |
| S7 | Only **two** Apple surfaces have a clock: `com.culebraluxe.apple-sync` (iMessage, 08:00 and 18:00) and `calendar-sync`. Contacts, calls/FaceTime, Gmail and Apple Mail each have one working command and no scheduled job. | `launchctl list \| grep culebraluxe`; `plutil -p ".../com.culebraluxe.apple-sync.plist" \| grep -A8 StartCalendarInterval` |
| S8 | The other exports are stale because nothing runs them, not because they broke: `contacts-export.json` 2026-09-22 15:33, `calls.jsonl` 2026-09-28 16:27. | `ls -la .../public/upload/data/apple-messages-export/` |

## 2. HOLDS — do not act on these

| # | Held | Who holds it | What an agent must do |
| --- | --- | --- | --- |
| H1 | `pnpm apple:sync:install` | a property of the machine | Do not run it casually: it rebuilds and re-signs the launcher, which **silently revokes FDA**, and that is exactly what broke 10-03. If it is ever needed: install first, then grant FDA. |
| H2 | Any PROD write: the intake is `APP_ENV=production`; the Contacts, calls, Gmail and Apple Mail loads are the same class. | Captain | Ask per run. "Run it" was given for run 4 only. |
| H3 | The production deploy of the map fix (`e392b9f5`) | Captain | Nothing ships without the word "deploy". |

## 3. WHERE TO LOOK — task → the one place

| Your task | Read | The files you touch |
| --- | --- | --- |
| Confirm how today's run ended | S6's two commands | `/Users/Shared/dev/build/logs/apple-sync.log` |
| Understand the FDA trap, and the 5-second launchd probe | `docs/agent/MEMORY.md`, entry 2026-10-04 | — |
| Give Contacts / calls / mail a clock | S7, then copy the pattern | `scripts/apple-sync-agent.mjs` (install/status/run agent), `~/Library/LaunchAgents/com.culebraluxe.apple-sync.plist` (calendar job) |
| The sync itself | `scripts/apple-sync.sh` (the deployed copy is byte-identical) | `apple-messages-export/` for the Swift exporter |

## 4. DONE — what landed, with the receipts

| Commit | What it changed | The gate that ran |
| --- | --- | --- |
| `e392b9f5` | the property map fix: `maybe_init_map` gated on `PropertyTab::Map` | `cargo test -p ui` 157/0; `pnpm ui:check` clean; CDP proof on :3000 and :3100 |
| `7dc0001b` | the map lesson in `docs/agent/MEMORY.md` | docs only |
| `e69f7e0b` | this finding's durable lesson in `docs/agent/MEMORY.md` | docs only, pushed to `origin/main` |
| not a commit | run 4 of `com.culebraluxe.apple-sync`: fresh export + validation | the log lines quoted in S6; the intake half is a §5 line until its tally is read |

## 5. NOT VERIFIED — the honest gaps

- **The PROD intake's own tally was not read before this file was written.** The export and the validation were; the intake was still executing (runtime 7:51 at write time). Read the `PROD intake tally:` line, `sync complete`, and `launchctl print … | grep 'last exit code'` (expect 0) before calling the run done.
- `verify-tcc` was never run from a shell after the grant — and would prove nothing if it had been: it inherits the Terminal's own FDA. The launchd probe (S3) is the evidence.
- How long a healthy intake takes is unmeasured: the log holds only the 10-03 failures, so "slow" cannot be judged from this file.
- Nothing was checked on the DEV side; this run writes PROD by design.
- The launcher still carries the dead `Documents/Culebraluxe-web` fallback (S5) — untouched on purpose, because rebuilding costs the FDA grant.

## 6. OPEN — the next actions, in order

1. Read run 4's verdict: `grep -E 'tally|sync complete|intake FAILED' /Users/Shared/dev/build/logs/apple-sync.log | tail -3` and `launchctl print gui/501/com.culebraluxe.apple-sync | grep 'last exit code'`. Finished when the tail shows `sync complete` and the exit code is 0; if `intake FAILED`, the export package is preserved and the next run replays.
2. Decide the cadence for Contacts and calls/FaceTime (S7, S8): either one weekly LaunchAgent running `pnpm ods:load:calls` (which covers contacts and calls) plus `pnpm contacts:sync:prod`, or accept manual runs. The two template files are in §3.
3. Optional tidy: rebuild `apple-messages-export/` so `main.swift:33`'s fallback points at the real repo, then re-grant FDA once (H1's order — install first, grant second).
4. Ship the map fix when the Captain says "deploy": production still serves a wasm bundle with no map code and 404s `/api/rust-ui/maps-key`.

## 7. ASK THE OWNER

- **"Do you want Contacts and FaceTime on a clock?"** — yes: one weekly LaunchAgent for `pnpm ods:load:calls` (plus `contacts:sync:prod`) lands in the same shape as apple-sync; no: they stay manual and stale.
- **"Deploy the map fix?"** — yes: `pnpm deploy:prod`, then `/api/rust-ui/maps-key` must answer 200 in production; no: nothing moves.
- **"Rebuild the launcher to clear the last dead path?"** — yes: rebuild and redeploy, then you re-grant FDA once (the same 5-second click); no: the string stays inert behind the plist env.

