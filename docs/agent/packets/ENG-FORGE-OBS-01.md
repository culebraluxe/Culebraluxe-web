# ENG-FORGE-OBS-01 — Forge signals: QA failure with no durable reason + stale queue slot

**Status:** packet written, not started. Priority High · ENGINEERING / TECH / SCOPED.
**Filed:** 2026-09-11. Packet written 2026-09-11.

## Goal

Make the last two Forge failure modes answerable as **one query instead of archaeology**,
and report them through the existing observer/scorecard seam.

## Why (the two incidents)

1. **QA failure with no durable reason.** A QA failure was recorded without a durable
   reason, and the cost was **four wasted repair cycles on WS-05** before anyone could
   say what had actually failed.
2. **Stale queue slot.** Roughly **twelve queue slots were cleared by hand** because a
   Ready/Claimed/Running item sat there with no active run behind it. This is the same
   class of ambiguity that makes the board unable to explain itself.

Both are *absence* failures: nothing crashed, nothing errored, something was simply not
recorded. That is why they are invisible today — the observer records what happened, and
these are cases where the answer is what is *missing*.

## Scope

Two signals, both derived from **durable facts alone**, both **read-only**.

### Signal 1 — QA failure with no durable reason

A run whose QA outcome is a failure while the durable reason fields are empty
(`failure_code`, `evidence_detail`, verdict/reason on the workflow evidence row).

Detection shape: one query over the run/evidence tables joining outcome against reason
fields; a row with a failing outcome and a null/empty reason is the signal. Severity
`error` when it occurs on a run that reached QA.

### Signal 2 — stale queue slot

An `agent_work_item` resting in `Ready`/`Claimed`/`Running` with no active run behind it
(or untouched beyond a bounded age).

Detection shape: one query comparing queue state against run activity and `updated_at`.
The threshold is a constant with a name, not a magic number inline.

## Where it lives

Through the **existing** seam — no new store, no new table, no UI:

- `workflow_app/forge/forge-observer/` — the append-only trace events.
- `workflow_app/forge/forge-alerts/rules.ts` — the rule set (`info | watch | hold-recommend`).
- `workflow_app/forge/forge-scorecard.ts` — the reporting surface (`pnpm forge:scorecard`).

**Phase 1 is log-only.** Neither signal may gate a run. A `hold-recommend` is a
recommendation, not a HOLD — the runner still owns that decision.

## Acceptance

1. A QA failure with no durable reason is detectable from durable facts alone (one query / one alert).
2. A stale Ready/Claimed/Running slot with no active run is detectable the same way.
3. Both surface through the observer/scorecard seam — no new store, no UI required, and neither may gate a run (phase 1).
4. **Proof: fixtures for each signal, present and absent.** The negative case matters most — no false alarms.
5. Evidence cites the motivating incidents (WS-05 four wasted repair cycles; ~12 hand-cleared slots).

## Verification

- Assay: `pnpm exec tsc --noEmit && pnpm test:forge:engine`
- Observer/alerts suite: `pnpm test:observer`
- A live `pnpm forge:scorecard` read against PROD, which must not crash on a clean board.

## Notes and limits

- Both signals are **derived**, not stored. Nothing new is persisted, so nothing new can drift.
- The stale-slot threshold is a judgement call; it is declared as a named constant so it
  can be tuned without touching logic.
- This does not repair either condition — it makes them **visible and countable**. Repair
  is a later story, deliberately.
- Related: `ENG-FORGE-SYNC-01` (board truth), `TECH-DEBT-01` (PROD guard wiring).
