// ---------------------------------------------------------------------------
// ENGINEERING QUEUES — the working-screen contract (iteration 1, static data).
//
// Four queues, two owners, and one stats strip. Names are the captain's, from
// 2026-09-12, and they encode a distinction the old screens never made:
//
//   bench   — WORK BENCH: the human's active work for the day. YOURS.
//   ready   — ENGINE READY: stories MARKED to be handed to Forge. The engine's.
//   running — RUNNING: the batch the machine has queued and is executing now.
//   results — RESULTS: finished ATTEMPTS. A row is a RUN, not a story, because a
//             story that reaches HOLD is replayed — 454 runs over 122 stories.
//             The outcome (DONE / ERROR / HOLD) is a STATE ON THE ROW, which is
//             why HOLD is not a queue of its own.
//
// Grain matters here and it is deliberate: bench/ready hold stories, running and
// results hold attempts. The story id is the link between the two grains.
// ---------------------------------------------------------------------------

export type QueueKey = 'bench' | 'ready' | 'running' | 'results'

/** What the engine spat out for a finished attempt. HOLD is a RESULT, not a queue. */
export type RunOutcome = 'DONE' | 'ERROR' | 'HOLD' | 'INTERRUPTED'

export type QueueCard = {
  /** Story id — the stable identity (`property.id` discipline: never the title). */
  id: string
  title: string
  workstream: string
  /** Board status as it stands today (Planned / In Progress / Hold / Complete). */
  status: string
  priority: string
  completion: number
  /** Where the card sits right now. Moving it is the ownership switch. */
  queue: QueueKey
  /** results only: the outcome of THIS attempt. */
  outcome?: RunOutcome
  /** results only: which attempt this was, and how it ended. */
  attempt?: number
  endedOn?: string
  /** results only: the run to open in the Flight Recorder on double-click. */
  /**
   * The STORY this card stands for - NOT an instance id.
   *
   * A story gets a fresh process instance every attempt, so a card cannot know "the" instance id;
   * it knows the story. The recorder resolves story -> latest instance and rewrites the URL to the
   * canonical instance UUID. The field used to be called `instanceId` and carried values like
   * `FORGE-SMITH-DOOR-01-demo`, which is why the screen could only answer 503/400: it was asked to
   * read a trace for something that had never been an instance.
   */
  storyId?: string
  /**
   * The REAL process instance id for this card's attempt, when the engine ledger knows it.
   *
   * Engine-lane cards (RUNNING / RESULTS) carry it, so a double-click opens THAT attempt's trace
   * directly. Human-lane cards only know their story, so they leave it unset and the recorder
   * resolves story -> latest instance instead. The preference order lives in `openRecorder`.
   */
  instanceId?: string
}

export type MetricTile = {
  label: string
  value: string
  caption: string
}

/** Cumulative, cost-accounting style — no window. See the fixture for provenance. */
export type RunStats = {
  asOf: string
  runs: number
  stories: number
  pctComplete: number
  pctHold: number
  pctRerun: number
  pctFail: number
  /** The average hides this, so the strip names it. */
  worstOffender: { id: string; runs: number }
}

export type LifecycleBucket = {
  key: string
  label: string
  count: number
  caption: string
  /** A few rows, for the expandable band. The full list is a wiring job. */
  sample: { id: string; title: string; status: string }[]
}

export type EngineeringQueuesModel = {
  title: string
  eyebrow: string
  subtitle: string
  asOf: string
  tiles: MetricTile[]
  stats: RunStats
  cards: QueueCard[]
  lifecycle: LifecycleBucket[]
}
