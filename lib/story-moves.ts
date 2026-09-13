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
export type StoryBucket = 'backlog' | 'open' | 'bench' | 'engine' | 'closed' | 'next'

/** Buckets a human can DRAG between (the sorter's columns). */
export const SORTER_BUCKETS: StoryBucket[] = ['backlog', 'open', 'bench', 'engine']

/** Buckets reachable only as a deliberate choice (no drop target exists for them). */
export const DELIBERATE_BUCKETS: StoryBucket[] = ['closed', 'next']

/**
 * Where a move may come FROM, per destination.
 *
 * ENGINE QUEUE is deliberately absent from every `from` list: nothing drags out of
 * it. RUNNING and RESULTS are not buckets at all — the engine owns them and they
 * are never a drop target.
 */
export const MOVES: Record<StoryBucket, StoryBucket[]> = {
  // BACKLOG is the source pool: nothing comes back INTO it except from OPEN or the
  // bench, where you parked something you are not doing after all.
  backlog: ['open', 'bench'],
  // OPEN is the hub: out to the bench, straight to the engine, or parked/deferred.
  open: ['backlog', 'bench', 'engine', 'closed', 'next'],
  // The bench can hand over, put work back into the queue, or park/defer it.
  bench: ['open', 'backlog', 'engine', 'closed', 'next'],
  // One way. The engine owns it from here.
  engine: [],
  // Closed and deferred stories are outcomes: they can be REOPENED into the queue,
  // which is the only sane way back from an accidental close.
  closed: ['open', 'backlog'],
  next: ['open', 'backlog'],
}

export function canMove(from: StoryBucket, to: StoryBucket): boolean {
  return MOVES[to]?.includes(from) ?? false
}

/**
 * Which story STATUS a bucket means.
 *
 * Only OPEN is a choice: its bucket holds six statuses (In Progress, Partial,
 * Ready, Blocked, Hold, Failed). OPEN means READY — the neutral "open, not
 * started, not blocked" state — because In Progress should keep meaning "I am
 * working it", which is what the WORK BENCH is for. The rest are single-status
 * buckets, so they are forced, not chosen.
 *
 * WORK BENCH and ENGINE QUEUE have NO status: the bench is an intent row in
 * storyboard_active_work (it never changes status) and the engine queue is an
 * agent_work_item. That is why the move layer has to know where each bucket lives.
 */
export const STATUS_BY_BUCKET: Partial<Record<StoryBucket, string>> = {
  open: 'Ready',
  backlog: 'Planned',
  closed: 'Complete',
  next: 'Deferred',
}

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
  if (bucket === 'open') return 'Clears any engine outcome (Hold / Failed / Blocked)'
  if (bucket === 'engine') return 'Hands the story to Forge — the engine owns it from here'
  if (bucket === 'bench') return 'Adds it to your Work Bench for today (no status change)'
  return null
}
