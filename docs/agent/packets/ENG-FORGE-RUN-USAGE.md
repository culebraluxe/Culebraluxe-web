# ENG-FORGE-RUN-USAGE — capture model token/cost usage per run into Neon

## Why

Calibration and regression analysis are impossible today: `storyboard_story_run`
has `tokens_input` / `tokens_output` / `cost_usd` / `cost_widgets`, but **nothing
writes them** (0 tokens across 204 PROD runs; only 52 widget rows). Without usage
per run there is no spend accounting, no difficulty/cost calibration, no regression.

## Key finding (the unlock)

The harnesses know usage and expose it — we just never ask:

- **OpenCode** persists a per-session record locally
  (`~/.local/share/opencode/opencode.db`, `session` table) with columns:
  `cost`, `tokens_input`, `tokens_output`, `tokens_reasoning`,
  `tokens_cache_read`, `tokens_cache_write`, plus `directory` (the worktree),
  `model`, `time_created`. => per-run usage is queryable by the run's worktree.
- OpenCode CLI also exposes the supported surface:
  `opencode export <sessionID>` (session JSON) and
  `opencode stats [--days N] [--models N] [--project]`.
- **DeepSeek (`dsh`)** bin is not on PATH in this checkout — its usage path is a
  separate open question (see Stop conditions).

## Resolved: OpenCode V2 — and the private table is no longer read (2026-10-01)

`ENG-FORGE-OPENCODE-V2` migrated the harness to the installed OpenCode v2.0.21 and closed
both of this packet's open ends. The finding above is kept as the archaeology; this is what
is true now.

- **Stop condition 1 is FIXED.** The session id is no longer inferred from a marker. V2
  reports the session it used on *every* event of `run --format json`, Forge captures that
  id, and each later turn resumes `--session <that id>` explicitly rather than relying on
  the directory-global `--continue`. A run now maps to its session by construction, not by
  searching for the newest session in a directory.
- **The `session` table is NO LONGER READ.** It was an implementation detail: a schema,
  column names and a file path Forge does not own. Accounting moved onto the surfaces the
  vendor documents — `session export <id> --standalone` for the totals (authoritative, and
  preferred), and `session list --standalone --format json` for finding the lane's own
  session. A source guard (`forge_never_reads_opencodes_private_session_store`) fails if any
  `rust/forge/src` file reaches for `sqlite3`, `opencode.db` or `FORGE_OPENCODE_DB` again.
- **`opencode stats` is still not used**, deliberately. Per-run accounting needs
  per-*session* numbers; `stats` is an aggregate and cannot attribute spend to a run.
- **Child sessions are DECLARED, not silently dropped.** V1 summed child sessions into
  their root through a recursive query over the private table. No supported V2 interface
  exposes that linkage (the list is top-level only; the export has no parent/child field),
  so it cannot be enumerated — and `opencode debug agents` reports no agents on this build,
  so a Forge turn spawns none. A test trips the moment V2 reports the linkage.
- **Stop condition 3 still stands, and is now enforced in the parser.** `step_finish` sums
  and the export are read strictly: an absent cost or token count is `None` — unmeasured —
  never `0`. An unreadable export and a non-zero exit both read as unmeasured.
- **One honest caveat:** the live 2.0.21 build does not reliably emit a `step_finish` for a
  turn's TERMINAL step, so the event-stream sum is a LOWER BOUND. It is only the fallback
  for when `session export` cannot be read (a failed turn often has no other reading), and
  the preferred export figure is never replaced by it.

## Plan

1. **Capture (opencode):** after a role run, resolve that run's session and read
   `tokens_input`, `tokens_output` (+ reasoning/cache if useful) and `cost`.
   Prefer the supported CLI (`opencode export <sessionID>` / `stats`); fall back
   to the local `session` table keyed by `directory` = the run worktree. Record
   the source used.
2. **Thread:** carry usage into the role result (additive field).
3. **Persist (always-write):** add a setter `setStoryRunUsage(runId, {tokensInput,
   tokensOutput, costUsd})` and call it in a `finally`/observer so usage lands
   even when the node HOLDs or the worker is killed — this is the "abstract-class
   finally" guarantee: **the write must never be gated on success.**
4. **Tests:** pure parser test (parse a session usage payload) + setter shape test;
   one live run asserts the run row's tokens are non-null afterward.

## Acceptance

- After a real role run, that story run's `tokens_input`/`tokens_output` are
  non-null and match the OpenCode session usage; `cost_usd` set when available.
- Usage is written on the failure/HOLD path too (finally semantics), not only on success.
- No new identity/model nouns leak into the generic engine; capture lives in the
  harness adapter + db seam.

## Stop conditions (report, don't invent)

1. OpenCode has no stable way to map a run to its session (id not captured today —
   session continuity tracks a marker, not an id) => report and prefer capturing
   the session id at spawn time.
2. `dsh` (DeepSeek) exposes no usage => capture what exists (opencode) and report
   the DS gap; never fabricate tokens.
3. Never write heuristic/estimated tokens into these columns — null is honest;
   a fake number poisons calibration.

## Test strategy

Targeted unit tests for the parser + setter; one live opencode run to confirm the
numbers populate. No full regression.
