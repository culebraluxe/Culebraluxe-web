# COCKPIT QA — the captain's script (2026-09-14)

Follow THIS rather than improvising: it touches only things that are safe to touch, and every step
says what you should see. `pnpm forge:batch:status` is READ-ONLY — run it as often as you like,
before and after each step.

## BEFORE YOU START — two facts that decide the script

1. **The unattended worker is running every 3 minutes.** Anything in the ENGINE QUEUE gets claimed
   and RUN FOR REAL (tokens, code, commits) within about 3 minutes. So do not hand junk to
   ENGINE RUN Q, and do not press `Run batch now` unless you mean it.
2. **Staging is free.** ENGINE BATCH fires nothing. Moving a card there is reversible and costs
   nothing — it is the safe place to test.

## STEP 0 — the state now

    pnpm forge:batch:status

Expect: `staging batch: none`, `no batches recorded yet`, `empty — the engine is idle`,
`work bench: 12`, `board vs table: agree`. Keep this output; it is the "before".

## STEP 1 — you are on the new build

Hard refresh (Cmd+Shift+R) and look at the far right corner of the Cockpit.

Expect: `V2 · f61ebbc` (or a later sha). If it reads something else the deploy has not landed, and
nothing below will behave as described.

## STEP 2 — clear the junk off the bench (your actual job)

WORKBENCH panel header → **`clear bench (12)`** → confirm.

Expect:
- a result line: `Cleared 12 stories off the bench. Statuses untouched.`
- `WORKBENCH (0)`, and the SORTER's WORK BENCH column empty
- **OPEN still reads 2** — correct: no status changed
- the five `Complete` V5 stories are still `Complete`

    pnpm forge:batch:status

Expect `work bench: 0`, everything else unchanged. (This is why the button exists: moving those
five finished stories to OPEN would have un-finished them.)

## STEP 3 — stage a card (the Kanban → table sync)

Pick `ENG-FORGE-V5-31` (a V5 story, not urgent). On its card use **`move to…`** → **`Batch`**.

Expect: the card moves to ENGINE BATCH and the board says `staged in the table (1 in the batch)`.
The button now reads `Run batch now (1)`.

**Do NOT press `Run batch now`** — that would start a real engine run.

    pnpm forge:batch:status

Expect: `staging batch <id> · 1 story(ies)`, `batched 1`, `board vs table: agree`, and the
ENGINE QUEUE **still empty** — that is the proof that staging fires nothing.

## STEP 4 — unstage it (the row leaves with the card)

Same card, **`move to…`** → **`Backlog`**.

Expect: the card back in BACKLOG, status back to `Planned`.

    pnpm forge:batch:status

Expect: `staging batch: none — nothing is staged right now`, `batched 0`, `agree`.

## STEP 5 — only if you have real work for overnight

Stage the stories you actually want run, set the time field next to `Run batch now`, press
**`Schedule (n)`**.

Expect: a roster line reading `scheduled <date time>` with `n/n stories` and a `cancel` link beside
it. At that time the unattended worker fires it and the engine runs the stories one at a time.

    pnpm forge:batch:status

`Scheduled  fires <time>` is the row that will fire. Cancel it if you change your mind — nothing has
been dispatched until it fires.

## WHAT NOT TO DO (and why)

- **Do not press `Run batch now` on junk.** It dispatches immediately and the worker claims it within
  ~3 minutes. Pulling a story back out of ENGINE RUN Q does withdraw the request, but only while it
  is still `Ready`, and the race is those same 3 minutes.
- **Do not hand a story to ENGINE RUN Q to "see what happens"** — same reason.
- Do not edit `.env.local`; none of these steps need it.

## IF SOMETHING IS WRONG, THIS IS THE REPORT I NEED

1. The corner sha (`V2 · …`).
2. The step number.
3. What you clicked and what the screen said (the note under the board, or the refusal).
4. The `pnpm forge:batch:status` output.

Those four are enough to reproduce almost anything on this screen without guessing.
