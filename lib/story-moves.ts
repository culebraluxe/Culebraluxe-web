// ---------------------------------------------------------------------------
// STORY MOVES — the legal transitions between the SORTER's columns, as DATA.
//
// Why data and not UI logic: the same rules have to hold for a drag on the board,
// a button on the story detail, and whatever comes next. Encode them once, here,
// and every surface asks the same question.
//
// The captain's rules, 2026-09-12:
//
//   1. The sorter reads BACKLOG -> OPEN -> WORK BENCH -> ENGINE QUEUE.
//   2. THE ENGINE GATE IS ONE WAY. "Once the engine has control that's it, no
//      messing with it." So nothing moves OUT of ENGINE QUEUE by hand. The only
//      exit is the engine's own cancel — which is an engine action, not a gesture.
//   3. Work reaches the engine from the WORK BENCH **or** directly from OPEN.
//   4. CLOSING and DEFERRING are DELIBERATE ACTS, not drags: they are outcomes, not
//      places, which is why CLOSED and NEXT VERSION are not sorter columns. They
//      stay reachable from ACTIVE as explicit choices.
// ---------------------------------------------------------------------------

/** The sorter's columns, left to right. */
export type StoryBucket = 'backlog' | 'open' | 'bench' | 'batch' | 'engine' | 'closed' | 'next'

/** Buckets a human can DRAG between (the sorter's columns). */
export const SORTER_BUCKETS: StoryBucket[] = ['backlog', 'open', 'bench', 'batch', 'engine']

/** Buckets reachable only as a deliberate choice (no drop target exists for them). */
export const DELIBERATE_BUCKETS: StoryBucket[] = ['closed', 'next']

/**
 * Where a move may GO TO, per source bucket.
 *
 * ENGINE QUEUE is deliberately an EMPTY list: nothing drags out of it. RUNNING and
 * RESULTS are not buckets at all — the engine owns them and they are never a drop
 * target.
 *
 * (This comment previously said "comes FROM, per destination", which was the exact
 * inverse of the data below and of `canMove`. The inversion shipped: every entry
 * to ENGINE was refused and every exit from it was allowed, so dragging a story
 * into ENGINE QUEUE did nothing. Corrected 2026-09-12.)
 */
export const MOVES: Record<StoryBucket, StoryBucket[]> = {
  // BACKLOG is the source pool: nothing comes back INTO it except from OPEN or the
  // bench, where you parked something you are not doing after all.
  // BACKLOG can go anywhere a story can plausibly go next: into the queue, onto the bench, staged
  // into the engine batch, or straight to the run queue when the captain means it.
  backlog: ['open', 'bench', 'batch', 'engine'],
  // OPEN is the hub: out to the bench, into the next engine batch, straight to the engine, or
  // parked/deferred.
  open: ['backlog', 'bench', 'batch', 'engine', 'closed', 'next'],
  // The bench can stage into the batch, hand over, put work back into the queue, or park/defer it.
  bench: ['open', 'backlog', 'batch', 'engine', 'closed', 'next'],
  // ENGINE BATCH is STAGING: stories wait here until the operator sends the batch. Nothing leaves it
  // on a timer and nothing in it has been dispatched - `Batched` is not the dispatch status. It can
  // send to the engine (the deliberate act) or be pulled back out.
  batch: ['engine', 'open', 'backlog', 'bench'],
  // One way. The engine owns it from here.
  engine: [],
  // Closed and deferred stories are outcomes: they can be REOPENED into the queue, and deferred work
  // can go STRAIGHT to the engine batch or the run queue - a story parked for a later version is
  // still a candidate the captain may want to hand over deliberately.
  closed: ['open', 'backlog'],
  next: ['open', 'backlog', 'batch', 'engine'],
}

export function canMove(from: StoryBucket, to: StoryBucket): boolean {
  // MOVES is keyed by SOURCE, so read the source's list of allowed targets.
  // Reading `MOVES[to]` (the bug) inverted the whole gate: nothing could enter
  // ENGINE and anything could leave it.
  return MOVES[from]?.includes(to) ?? false
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
