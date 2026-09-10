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
