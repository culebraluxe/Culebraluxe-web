# COCKPIT SMOKE TEST — the board, end to end

Written 2026-09-14 against the real PROD board, for the captain's own QA. Every
"expect" below is what the screen actually does, not what it ought to do: each line
was read from the code that produces it.

## State when this was written (PROD, read-only probe)

- 323 stories: 248 `Complete`, 35 `Deferred`, 33 `Planned`, 5 `Hold`, 2 `In Progress`.
- **WORK BENCH: 12 stories.** `ENG-FORGE-V5-21/22/28/29/30/31` and `CRM-28` are `Planned`;
  `ENG-FORGE-V5-23…27` are `Complete` — five finished stories sitting on today's bench.
- OPEN: 2 stories, both genuinely in progress — `ENG-FORGE-TURN-VISIBILITY-01`,
  `ENG-FORGE-DOCTOR-01`.
- ENGINE: **idle** — 0 open work items, 0 stale claims.

## THE TRAP (read this before clearing the bench)

"Do not" move each bench card to **OPEN** to clear the bench. OPEN is a STATUS write
(`In Progress`), so doing that to the five `Complete` stories would **un-finish finished
work** to tidy a list. The bench is an intent row and nothing else, and that is what the
`clear bench` button uses.

## PART 1 — clear the junk (the captain's own gesture)

1. Cockpit (`/portal/tech`), WORKBENCH panel header, click **`clear bench (12)`** and confirm.
2. Expect: a result line — `Cleared 12 stories off the bench. Statuses untouched.`
3. Expect: WORKBENCH `(0)`, and the SORTER's WORK BENCH column empty.
4. Expect: **OPEN still reads 2.** This is correct and is the whole point — no status changed.
   The cleared `Planned` stories do NOT appear in OPEN; `Planned` belongs to BACKLOG, and that
   is where they were and still are.
5. Expect: the five `Complete` stories are still `Complete`. Reversing this is just adding a
   story back to the bench; nothing was deleted.

## PART 2 — the smoke: OPEN → WORK BENCH → ENGINE RUN Q

Use `ENG-FORGE-DOCTOR-01` (in OPEN, engine idle, safe to undo).

6. In the SORTER, on the `ENG-FORGE-DOCTOR-01` card, click **`→ Bench`**.
   Expect: it leaves OPEN, appears under WORK BENCH, and WORKBENCH reads `(1)`. Its status is
   still `In Progress` — the bench never changes status.
7. On that card, click **`→ Run Q`**.
   Expect: it leaves WORK BENCH, appears under **ENGINE RUN Q**, and disappears from the WORKBENCH
   panel (handing work to the engine takes it off the daily list).
   Expect: this is the ONLY move on the board that starts anything. It writes status `Ready`,
   and `Ready` is the engine's dispatch trigger: a real `agent_work_item` is created.
8. Confirm with the engine's own record, not the card:
   `APP_ENV=production node --env-file=.env.local --import tsx scripts/probe-agent-work-state.ts`
   Expect: `OPEN WORK ITEMS … : 1` naming `ENG-FORGE-DOCTOR-01` (it was 0 before step 7).
9. Expect: the ENGINE panel's QUEUED lane shows it too — that lane is the engine's waiting list,
   the same rows as step 8.

If a drop or a button is ever refused, the reason is printed on the screen above the board. A
refusal is a rule talking, and the rules live in `lib/story-moves.ts`.

## PART 3 — undo the smoke (one command)

10. `pnpm forge:story:reset ENG-FORGE-DOCTOR-01 reset --force`
    This cancels the open work item, interrupts any claim, and sets the story back to `Planned`.
11. Re-run the probe from step 8: expect `OPEN WORK ITEMS … : 0`.
12. Put the story back where it belongs with the board itself: `→ Open` on its card.

## What this does NOT prove

- Nothing about a live engine run. The smoke proves the HANDOFF (the row, the trigger, the work
  item). Running the worker is a separate act: `pnpm forge:engine`.
- Nothing about the DEV_OPS receipt, which is still the last door on a release-bearing story.
