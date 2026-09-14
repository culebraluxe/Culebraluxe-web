// ---------------------------------------------------------------------------
// STORY MOVES — the SORTER's columns, and NO GATE.
//
// HISTORY, because it explains the file: this module used to encode a MOVES table
// of "legal transitions" — nothing could leave the engine's column, closed stories
// could only be reopened, backlog could not be closed. It accumulated comments
// defending the rules and tests locking them in.
//
// The captain, 2026-09-14, on what that felt like to use:
//
//   "i dont want any rules i dont understand why this is so complicated these are
//    just sticky notes on a kahnban in real life i can just pick a sticky note off
//    the white board kahnban and move it where ever i want ... this is so annoying"
//
// He is right. A board whose cards refuse to move is a board you cannot use, and a
// rule you have to learn before you can tidy a list is not a feature. So there are no
// legal-transition rules any more: EVERY COLUMN MAY GO TO EVERY OTHER COLUMN.
//
// What remains is not a permission, it is a FACT about what a column DOES:
//   - a column that is a lifecycle state writes that status (`In Progress`, `Planned`,
//     `Complete`, `Deferred`);
//   - the bench is an INTENT row and changes no status;
//   - ENGINE BATCH stages (`Batched`, harmless, dispatches nothing);
//   - ENGINE RUN Q HANDS THE STORY OVER: status `Ready`, which is the engine's own
//     dispatch trigger, so this is the one move that starts machine work;
//   - LEAVING ENGINE RUN Q WITHDRAWS the queue entry it created (see
//     `withdrawQueuedAgentWork`), so pulling the note back takes the request back with
//     it. Un-ringing a bell that has not rung yet; a story the engine is already
//     executing is reported, not silently yanked.
//
// This module therefore holds VOCABULARY (column names, their statuses, their
// consequences), not permission. `canMove` survives only so callers keep one question
// to ask — and it now answers "anywhere but where it already is".
// ---------------------------------------------------------------------------

/** The sorter's columns, left to right, plus the two deliberate outcomes. */
export type StoryBucket = 'backlog' | 'open' | 'bench' | 'batch' | 'engine' | 'closed' | 'next'

/** Buckets that are drawn as a column on the board. */
export const SORTER_BUCKETS: StoryBucket[] = ['backlog', 'open', 'bench', 'batch', 'engine']

/** Buckets reached as a deliberate act rather than a column (Close / defer, and NEXT VERSION). */
export const DELIBERATE_BUCKETS: StoryBucket[] = ['closed', 'next']

/** Every bucket, for "where may this go" — which is now "anywhere else". */
export const STORY_BUCKETS: StoryBucket[] = [...SORTER_BUCKETS, ...DELIBERATE_BUCKETS]

/**
 * MAY A STORY MOVE FROM ONE COLUMN TO ANOTHER?
 *
 * It may move anywhere it is not already. Two of the seven are not really statuses
 * (the bench is an intent row, ENGINE BATCH is staging) and one of them starts the
 * engine — but none of that is a gate: the caller is told what a move DOES by
 * `bucketSideEffect`, and the write paths above/below handle it. There is deliberately
 * no table of forbidden pairs to maintain, because the captain does not have one.
 */
export function canMove(from: StoryBucket, to: StoryBucket): boolean {
  return from !== to && STORY_BUCKETS.includes(from) && STORY_BUCKETS.includes(to)
}


/**
 * NORMALIZE A CALLER'S COLUMN NAME INTO A BUCKET — the boundary between what a VIEW calls a column
 * and what the RULES call a bucket.
 *
 * This exists because the two vocabularies drifted and the drift was invisible: the sorter's column
 * is `next-version` (the `StoryLifecycle` noun, shared with the story board) while the rules key is
 * `next`. Typing the string as `StoryBucket` in the action made TypeScript accept it and made
 * `MOVES['next-version']` `undefined`, so EVERY DRAG OUT OF NEXT VERSION WAS REFUSED — the rules
 * were right and unreachable, which is why the screen read as broken rather than the rules as wrong.
 *
 * It also absorbs DEPLOY SKEW: a browser holding the previous bundle keeps posting its old ids until
 * it reloads, and a rename must not turn into a mystery refusal in that window.
 *
 * Returns null for anything unrecognized, so the caller refuses BY NAME instead of quietly
 * normalizing a typo into a move.
 */
export function normalizeStoryBucket(raw: string): StoryBucket | null {
  const key = String(raw ?? '')
    .trim()
    .toLowerCase()
    .replace(/[\s_]+/g, '-')
  const ALIASES: Record<string, StoryBucket> = {
    // the lifecycle nouns the interface actually renders
    'next-version': 'next',
    'nextversion': 'next',
    // the sorter's column titles, in case a label ever becomes an id
    'work-bench': 'bench',
    'workbench': 'bench',
    'active': 'bench',
    'engine-queue': 'engine',
    'engine-q': 'engine',
    'engine-run-q': 'engine',
    'engine-running': 'engine',
    'backlog-batch': 'batch',
    'engine-batch': 'batch',
    'queued': 'batch',
  }
  if (key in ALIASES) return ALIASES[key]
  const direct = (SORTER_BUCKETS as string[]).concat(DELIBERATE_BUCKETS).includes(key)
  return direct ? (key as StoryBucket) : null
}

/**
 * Which story STATUS a bucket means.
 *
 * THREE OF THESE ARE FORCED by the lifecycle mapping; OPEN is the one free choice,
 * and it is now `In Progress` rather than `Ready` for a reason that is not cosmetic:
 *
 *   >>> `status = 'Ready'` IS THE ENGINE DISPATCH TRIGGER. <<<
 *
 * `agent_work_item_dispatch()` (migration 025, redefined in 146) fires on
 * `after insert or update of status on storyboard_story` and, the moment a story's
 * status becomes 'Ready', inserts an `agent_work_item` in state 'Ready' — i.e. it
 * QUEUES THE STORY FOR FORGE. So a column that writes 'Ready' is a column that
 * starts engine work. OPEN must NOT be that column: dragging a story into the open
 * queue would silently dispatch it. That is ENGINE QUEUE's job, and therefore
 * 'Ready' is reserved for the engine handoff, where the write is deliberate.
 *
 * WORK BENCH has NO status: the bench is an intent row in storyboard_active_work
 * (it never changes status). ENGINE QUEUE is left out of this map on purpose — its
 * write is the dispatch itself and is wired as its own deliberate act, never as a
 * side effect of a bucket move.
 */
export const STATUS_BY_BUCKET: Partial<Record<StoryBucket, string>> = {
  open: 'In Progress',
  backlog: 'Planned',
  closed: 'Complete',
  next: 'Deferred',
  // STAGING, not dispatch: 'Batched' changes nothing on the engine. The batch is sent by
  // `sendEngineBatchAction`, which writes ENGINE_DISPATCH_STATUS per story - deliberately, and only
  // when the operator says so.
  batch: 'Batched',
}

/** The status that fires the engine dispatch trigger. Reserved for ENGINE QUEUE. */
export const ENGINE_DISPATCH_STATUS = 'Ready'

/** True when the bucket is stored as a real status change. */
export function isStatusBucket(bucket: StoryBucket): boolean {
  return STATUS_BY_BUCKET[bucket] != null
}

/**
 * Side effects worth knowing before a write, so nobody is surprised:
 * a move to CLOSED forces completion to 100 (the repository does it), and a move
 * to OPEN overwrites whatever outcome the engine last recorded for that story.
 */
export function bucketSideEffect(bucket: StoryBucket): string | null {
  if (bucket === 'closed') return 'Sets completion to 100%'
  if (bucket === 'open') return 'Open work — no engine dispatch'
  if (bucket === 'engine')
    return 'DISPATCHES FORGE: sets status Ready, which queues the story. The engine owns it from here.'
  if (bucket === 'bench') return 'Adds it to your Work Bench for today (no status change)'
  return null
}
