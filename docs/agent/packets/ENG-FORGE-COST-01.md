# ENG-FORGE-COST-01 — Per-attempt usage capture: raw tokens + vendor-reported cost

**Status:** Planned (backlog) · **Priority:** Medium · **Surface:** TECH · **Test mode:** SCOPED
**Filed:** 2026-09-11, from the Forge Observer + scorecard session.

## Goal

Make per-attempt spend measurable **from a real source**, and flip the scorecard's two
telemetry lines from `NOT CAPTURED` to measured with a coverage count.

## Why this exists (the measured gap)

`pnpm forge:scorecard` reports what is captured and names what is not:

```
MODELS (harness-observed)
  deepseek/deepseek-v4-flash   175
  (unknown)                    144
TELEMETRY  model identity captured on 175/319 · widget cost captured on 175/319 ·
           raw tokens NOT captured (harness result carries no usage field) ·
           vendor dollars NOT captured — cost_usd is VENDOR-REPORTED only
```

So `model_used` and `cost_widgets` **are** captured; the gaps are **raw tokens** and the
**vendor dollar figure**. `OpenCodeRunResult` carries no usage field at all today.

## Preconditions

1. **Capture the OpenCode session id per attempt.** The client passes `--session` but never
   records a *new* session's id. This is the single prerequisite for everything below.

Verified CLI facts (from `opencode --help` / `run --help` / `stats --help`, 2026-09-11):

| Source | Scope | Verdict |
|---|---|---|
| `opencode export <sessionID>` | per-session JSON | ✅ the intended source (one attempt = one session) |
| `opencode stats` | aggregate (days/models/project) | ❌ cannot attribute a run |
| `opencode run --format json` | per-run raw JSON events | ⚠️ per-run, but changes stdout — the evidence pipeline parses stdout for assistant text and the `Tests:` line. Do **not** switch formats blindly. |

## Acceptance criteria

1. A run records its OpenCode session id durably.
2. `usage` exists on `OpenCodeRunResult` and is populated from `opencode export`.
3. `storyboard_story_run.tokens_input` / `tokens_output` / `cost_usd` are populated for a new
   run, with `cost_source` set (`vendor`).
4. `pnpm forge:scorecard` flips both lines to measured, with a coverage count.
5. **FORBIDDEN:** deriving `cost_usd` from `cost_widgets`. There is **no widgets→dollars rate**
   and none may be invented — `cost_widgets` stays the Forge-native unit, `cost_usd` is
   vendor-reported only. (See `docs/agent/MEMORY.md`, "cost units — do NOT convert".)
6. A failed export read degrades safely: usage unknown, the run is unaffected.
7. No change to how the agent is invoked (the run path stays `opencode run --model … --auto`).

## Assay

```
pnpm exec tsc --noEmit
pnpm test:observer
pnpm test:forge:engine
pnpm forge:scorecard
```

Plus one proof asserting a run's captured tokens match that run's own session export.

## Notes

- The observer is already live (phase 1, record-only) and cannot gate a run.
- `cost_widgets` is the unit runs are compared by today; this story adds the vendor lens
  beside it, never in place of it.
