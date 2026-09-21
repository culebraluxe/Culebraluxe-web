# WHAT THE COCKPIT IS FOR — the captain's own framing (2026-09-14)

Written because it was never written down, and because everything above this file in
the tree is a *piece* of it. The captain: "this is tricky stuff that had never been
connected before — it was the whole management of stories to story board to the engine
at the same time ... i probably should have started there ... this is the entire SDLC."

This is the north star for the Cockpit and the board. Code decisions that contradict it
are wrong even when they pass their tests.

## THE WHOLE THING, IN HIS WORDS

> "the entire forge project was developed to try to automate what you do so i could sleep
> and have code being produced. not every story will work in forge — we have a very
> effective collaboration to change things real time — but not every story is like that.
> for things that have a well written story it should be easy enough to identify the
> requirements, scope, classes and architect say this is how to do this. we spent about
> 100 hours trying to get the engine to work properly and the work to be passed via neon
> work orders but scoped to just 1 story at a time.
>
> and this whole cockpit is to understand the state of the storyboard and the state of
> the engine, and the final piece we are hammering now which is move stories across
> kahnban and then batch up a group of stories to run in engine together or just get a
> single story into the Q.
>
> those are why there are 2 engine lists if i was not clear enough about that:
> engine batch is just trying to say 'these are ready to go but i dont want to run them
> yet' — maybe save for a night run when its cheaper to run, or they dont need to go
> right now.
>
> the work we do in waking hours are the most difficult stories and we need back and
> forth collaboration."

## THE THREE MODES OF WORK (this is the whole design)

1. **WAKING HOURS — collaboration on the hard stories.** The captain and the agent in
   dialogue, changing things in real time. These stories are NOT engine material: they
   need judgement mid-flight. **This is WORK BENCH.** It is the human lane, and it is not
   a staging area for the engine — it is where the hard work happens.
2. **HAND IT OVER NOW — a single story, run immediately.** Well-specified stories only:
   requirements, scope, classes, and an architect saying how to do it. **This is ENGINE
   RUN Q.** Dropping a story here is the one board action that starts machine work
   (status `Ready` → the dispatch trigger → a real `agent_work_item`).
3. **LOADED, NOT FIRING — the ones that can wait.** "These are ready to go but I don't
   want to run them yet" — saved for an overnight run when it is cheaper, or simply not
   urgent. **This is ENGINE BATCH.** Staging writes `Batched`, which dispatches NOTHING,
   and it stays free until the captain presses Send.

Why TWO engine lists: they answer different questions. BATCH answers "what is ready and
waiting?" — a holding list the captain curates. RUN Q answers "what is the engine doing
or about to do?" — the live handoff. Collapsing them would destroy the ability to load
work up in advance and fire it later, which is the point of batching.

## THE INVARIANTS THIS DEPENDS ON (measured, not assumed)

- **ONE STORY AT A TIME.** The system-wide single-active lock is real
  (`legacy/db/agent-work.ts`, migration 025/028): the engine holds exactly one active work item.
  A batch of five is a queue of five one-at-a-time runs, not five parallel runs. Relaxing
  this is an ask-first change (see AGENTS.md).
- **BATCH FIRES NOTHING, BUT IT IS A REAL THING.** Staging writes `Batched` (dispatch-nothing status),
  and since migration 178 a batch ALSO has durable rows: `forge_batch` (label, `scheduled_for`,
  `fired_at`, who built it) and `forge_batch_item` (each member's state). So the screen can answer
  "what did I load up, when will it run, and how did the last one end?" instead of only showing its
  current state. Verified on PROD: scheduling a batch dispatched **0** work items; firing it dispatched
  exactly 1 per member; the story's batch history showed `Queued` with a timestamp.
- **THE BOARD SYNCS TO THE TABLE (Autosys model).** The captain ran trillion-dollar overnight cycles for
  15 years: "there is real time and batch overnight ... it only has to do one thing: grab the stories in
  that table and go ... if its in the table it goes". So `forge_batch_item` is the JOB STREAM and the
  board is its view: staging a card writes the row, taking the card out deletes it, and firing reads the
  ROWS (`fireStagingBatch`) instead of re-deriving a list from statuses. One open staging batch at a
  time; scheduling puts a time on that same row. Verified on PROD, net zero: stage → 1 row & 0 work
  items; unstage → 0 rows; run the table → 1 work item; withdraw → restored.
- **THE UNATTENDED WORKER SYNCS ITS CODE.** The launchd wrapper is deployed from
  `scripts/agent-worker-once.sh` and fast-forwards the checkout (`git pull --ff-only origin main`) before
  working. It was found STALE on 2026-09-14 (a deployed copy with no git sync at all, so an overnight run
  would execute whatever code happened to be checked out); reinstalled, `wrapper: synced
  sha256=77ea506cf062`, worker running.
- **A SCHEDULED BATCH FIRES ITSELF.** `fireDueForgeBatches()` runs at the top of every unattended
  worker pass, and the launchd scheduler already wakes that command every 3 minutes — so a batch
  scheduled for 02:00 runs at 02:00 with nobody awake, and there is no second daemon or cron entry to
  forget. (It deliberately does not depend on the launchd WRAPPER being redeployed.)
- **RUN Q FIRES EXACTLY ONE THING.** Writing `Ready` creates one work item (verified:
  `In Progress/0 items` → handoff → `1 Ready` item).
- **LEAVING RUN Q TAKES THE REQUEST BACK.** Pulling a story out withdraws the `Ready`
  item (`withdrawQueuedAgentWork`); a story the engine is already running is reported,
  not silently yanked.
- **UNATTENDED DRAIN EXISTS.** A launchd agent (`com.culebraluxe.agent-worker`) wakes
  every 3 minutes, runs `pnpm agent:work`, and drains queued work until Forge is idle —
  this is what makes "load a batch, go to sleep" real. ⚠️ Its DEPLOYED wrapper was found
  stale on 2026-09-14: the repo version fails closed on `git pull --ff-only origin main`
  before working, the deployed copy does no git sync at all, so an unattended run would
  execute whatever code was checked out when that copy was installed.

## WHAT IS NOT BUILT YET (honest gaps against the intent)

- **NOTHING TELLS YOU A STORY IS ENGINE-READY.** The intent says well-written stories are
  "easy enough to identify" — requirements, scope, classes, architect brief. Today the
  board will happily let you hand over a story that cannot survive the engine's own
  architect assessment; you find out by watching a run fail or HOLD. The engine already
  has the assessment (`legacy/workflow_app/forge/agents/architect/assess.ts`); the board does not
  show it before you press the button.
- **NO WAY TO SEE A BATCH WILL SUCCEED** before sending: no dry run, no estimate, no
  "these 3 of 5 passed readiness" pre-flight.
- **NO SENSE OF COST OR DURATION** per story or per batch, which is the stated reason for
  batching in the first place — "save for a night run when its cheaper to run" is now
  schedulable, but nothing tells you what the run will cost when it fires.
