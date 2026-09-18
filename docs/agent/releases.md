# Releases — what was actually built, deployed and probed

Append-only, written by `pnpm release` (`scripts/release-record.sh`). One row per attempt, failures included:
a missing row is not evidence of a clean release. `eligible=yes` means build, deploy and a live probe all
passed **for the sha in this row** — that, and only that, is a receipt. Everything else is a record.

| started (UTC) | ended (UTC) | sha | branch | tree | build | deploy | probe | eligible | outcome |
|---|---|---|---|---|---|---|---|---|---|
| 2026-09-18T02:35:26Z | 2026-09-18T02:35:31Z | `7697faa6d53c` | main | dirty | 1 | blocked-by-build | 1 | no | PROBE_FAILED |
| 2026-09-18T02:44:17Z | 2026-09-18T02:48:21Z | `d555058e5f7d` | main | clean | 0 | 0 | 0 | yes | ok |
| 2026-09-18T12:37:17Z | 2026-09-18T12:37:20Z | `c8fde5295b03` | main | clean | 1 | blocked-by-build | 1 | no | BUILD_FAILED |
