// ---------------------------------------------------------------------------
// SORTER BOARD — which column each card is drawn in, as a PURE FUNCTION.
//
// WHY THIS FILE EXISTS: "one story, one column" has now been the root cause of two
// separate wrong-card bugs on this board. Both times the columns were assembled
// independently - each with its own filter - and each filter was locally reasonable
// while the SET of them was not disjoint:
//
//   - the bench is an intent row, so a bench story kept its `In Progress` status and
//     was drawn in OPEN as well. Two cards, same id; the vendor's drop handler finds
//     cards by id, so moving one moved the other ("it pulls an adjacent story to the
//     right").
//   - `Ready` (a story handed to ENGINE RUN Q) maps to the OPEN lifecycle in
//     `lib/storyboard-data.ts`, so the handoff - the ONE move that starts machine
//     work - appeared in OPEN and ENGINE RUN Q at the same time. The captain called
//     this transition "the big thing", and it was the one column pair still able to
//     collide.
//
// So the assignment is made ONCE, here, in PRIORITY ORDER, with a single `claimed`
// set: the first column that wants a story gets it, and every later column is told the
// story is taken. Duplicates are not filtered out afterwards; they cannot be created.
// The order is also the honest answer to "who owns this story":
//
//   1. ENGINE RUN Q  - the engine has it (queued or running) or it was handed over
//   2. ENGINE BATCH  - staged, not dispatched
//   3. WORK BENCH    - you said you are working on it today
//   4. NEXT VERSION  - deferred
//   5. BACKLOG       - planned
//   6. OPEN          - everything else that is live (In Progress, Blocked, Hold, Failed)
//
// A pure function so the invariant is TESTABLE without a browser or a database:
// `workflow_app/tests/sorter-board.test.ts`.
// ---------------------------------------------------------------------------

export type SorterColumnId = 'backlog' | 'open' | 'bench' | 'batch' | 'next-version' | 'engine'

export const SORTER_COLUMNS: Array<{ id: SorterColumnId; label: string }> = [
  { id: 'backlog', label: 'BACKLOG' },
  { id: 'open', label: 'OPEN' },
  { id: 'bench', label: 'WORK BENCH' },
  { id: 'batch', label: 'ENGINE BATCH' },
  // "ENGINE RUN Q" is the captain's name for it: handing a story over here is what
  // makes the engine actually run it (`ENGINE_DISPATCH_STATUS` = Ready).
  { id: 'engine', label: 'ENGINE RUN Q' },
  { id: 'next-version', label: 'NEXT VERSION' },
]

export type SorterCard = {
  id: string
  column: SorterColumnId
  title: string
  status: string
  priority: string
  completion: number
  /**
   * The work's KIND (`lib/forge-kind.ts`) when the source knows it. `null` means "not read", which is
   * deliberately different from "no kind": the board should say nothing rather than print a default the
   * dispatch never wrote.
   */
  kind?: string | null
}

/** The shape of a story row these cards need. Structural, so tests need no database. */
export type SorterStory = {
  id: string
  title: string
  status: string
  priority: string
  completion: number
  /** Staged work carries its kind; a lifecycle story usually does not. */
  kind?: string | null
}

/** An engine ledger card: one per ATTEMPT, which is why the id carries the attempt. */
export type SorterEngineRun = {
  id: string
  /** Optional — a card that only knows its story id part may still stand for one (`RUN-1#2`). */
  storyId?: string
  title: string
  status: string
  priority: string
  completion: number
  /** `running` for a live attempt; anything else belongs to the engine panel's RESULTS lane. */
  queue: string
}

/** One entry of the engine's waiting list (`agent_work_item`). */
export type SorterQueuedWork = {
  storyId: string
  title: string
  state: string
}

export type SorterInput = {
  /** The cockpit projection: lifecycle panels, `backlog` / `open` / `next-version`. */
  panels?: Record<string, { groups?: Array<{ stories?: SorterStory[] }> } | undefined> | null
  /** `storyboard_active_work` — the bench. */
  activeWork?: SorterStory[] | null
  /** Stories whose status is `Batched` (staged for the engine, dispatches nothing). */
  batchStories?: SorterStory[] | null
  /** The engine ledger's cards. */
  engineRuns?: SorterEngineRun[] | null
  /** The engine's waiting work items. */
  queuedCards?: SorterQueuedWork[] | null
}

function panelStories(input: SorterInput, key: string): SorterStory[] {
  return (input.panels?.[key]?.groups ?? []).flatMap((g) => g.stories ?? [])
}

/** Every story the cockpit knows about, whatever lifecycle panel it sits in. */
function allPanelStories(input: SorterInput): SorterStory[] {
  const seen = new Map<string, SorterStory>()
  for (const panel of Object.values(input.panels ?? {})) {
    for (const group of panel?.groups ?? []) {
      for (const story of group.stories ?? []) {
        if (!seen.has(story.id)) seen.set(story.id, story)
      }
    }
  }
  return [...seen.values()]
}

/**
 * Build the SORTER's cards. Every story id appears in AT MOST ONE column — guaranteed by the single
 * `claimed` set, not by hoping the filters agree.
 */
export function buildSorterCards(input: SorterInput): SorterCard[] {
  const claimed = new Set<string>()
  const cards: SorterCard[] = []
  /**
   * `claimKey` is the STORY, `cardId` is the card. They are not the same thing for the engine's ledger
   * cards, whose ids carry the attempt (`RUN-1#2`): claiming by card id let a story's running attempt
   * AND its queued work item both draw, because `RUN-1#2 !== RUN-1#queued`. The sorter test caught it.
   */
  const take = (column: SorterColumnId, story: SorterStory, cardId?: string, claimKey?: string) => {
    const key = claimKey ?? story.id
    if (claimed.has(key)) return
    claimed.add(key)
    cards.push({
      id: cardId ?? story.id,
      column,
      title: story.title,
      status: story.status,
      priority: story.priority,
      completion: story.completion,
      kind: story.kind ?? null,
    })
  }

  // 1. ENGINE RUN Q — the engine's own record wins, because that is where the work IS.
  for (const run of input.engineRuns ?? []) {
    if (run.queue !== 'running') continue
    // The story id is what makes two engine cards for one story collapse to one sorter card.
    take('engine', run, run.id, run.storyId ?? run.id.split('#')[0])
  }
  for (const item of input.queuedCards ?? []) {
    take(
      'engine',
      {
        id: item.storyId,
        title: item.title,
        status: item.state,
        priority: 'MEDIUM',
        completion: 0,
      },
      `${item.storyId}#queued`,
    )
  }
  // A story whose status is `Ready` was handed over even if the engine has no row for it yet (or any
  // more). It belongs to the run queue by definition — drawing it in OPEN is what made the handoff
  // appear in two columns at once.
  for (const story of allPanelStories(input)) {
    if (story.status === 'Ready') take('engine', story, `${story.id}#handoff`)
  }

  // 2. ENGINE BATCH — staged, harmless, dispatches nothing.
  for (const story of input.batchStories ?? []) take('batch', story)

  // 3. WORK BENCH — the intent row. Beats every lifecycle column: you said it is today's work.
  for (const story of input.activeWork ?? []) take('bench', story)

  // 4. NEXT VERSION — deferred.
  for (const story of panelStories(input, 'next-version')) take('next-version', story)

  // 5. BACKLOG — planned.
  for (const story of panelStories(input, 'backlog')) take('backlog', story)

  // 6. OPEN — whatever is left that is live.
  for (const story of panelStories(input, 'open')) take('open', story)

  return cards
}

