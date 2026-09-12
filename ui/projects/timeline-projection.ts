import type { ILink, ITask } from '@svar-ui/react-gantt'

import type { ProjectPlan } from './model'

// ---------------------------------------------------------------------------
// Project -> SVAR Gantt tasks (PURE).
//
// The Timeline tab renders the real WBS structure of the selected project. Dates
// are the honest part of this file:
//
//  - a work node WITH a real `dueAt` is scheduled so the task ENDS on its due
//    date (a due date is a finish, not a start);
//  - a work node with NO due date is placed on a synthetic sample schedule and
//    the whole timeline is flagged `synthetic`, which the pane must disclose to
//    the user. It is never silently passed off as real scheduling.
//
// Dependency links are emitted only for a fully synthetic timeline, because a
// sequence we invented is not a dependency the project actually has. Real links
// must come from real WBS data, not from ordering.
//
// `ITask` / `ILink` are imported as TYPES ONLY, so this module stays runtime-pure
// and keeps running under plain tsx in the glass-box tier.
// ---------------------------------------------------------------------------

/** Sample-schedule anchor used only when a project has no real dates yet. */
const SAMPLE_SCHEDULE_START = new Date(2026, 8, 1)
const TASK_DAYS = 3

export type ProjectTimeline = {
  tasks: ITask[]
  links: ILink[]
  /** True when any task's dates are sample scheduling rather than project facts. */
  synthetic: boolean
}

const PROGRESS_BY_STATUS: Record<string, number> = {
  complete: 100,
  'in-progress': 60,
  waiting: 40,
  blocked: 0,
  'not-started': 0,
  dismissed: 0,
}

function addDays(value: Date, days: number): Date {
  const next = new Date(value)
  next.setDate(next.getDate() + days)
  return next
}

/**
 * Read a stored due date as the calendar DATE it is.
 *
 * Due dates are persisted as UTC midnight ('2026-09-10T00:00:00.000Z' — see the
 * WBS seeds). Reading the LOCAL parts of that instant would place it on the
 * previous day in any negative UTC offset, and CulebraLuxe runs at UTC-4: every
 * deadline would silently land a day early. So take the UTC parts and build a
 * local date from them, which keeps the calendar day the user actually set.
 */
function dateOnly(value: string | undefined): Date | null {
  if (!value) return null
  const parsed = new Date(value)
  if (Number.isNaN(parsed.getTime())) return null
  return new Date(parsed.getUTCFullYear(), parsed.getUTCMonth(), parsed.getUTCDate())
}

/**
 * Schedule one work node: end on its real due date when it has one, otherwise
 * lay it onto the sample schedule in plan order.
 *
 * Returns `duration`, never `end`: the Gantt derives the end from start+duration,
 * and supplying both is a redundant path through its date normalization.
 */
function scheduleFor(
  index: number,
  dueAt: string | undefined,
  anchor: Date,
): { start: Date; duration: number } {
  const due = dateOnly(dueAt)
  if (due) {
    // The due date is the FINISH, so the task starts TASK_DAYS-1 earlier.
    return { start: addDays(due, -(TASK_DAYS - 1)), duration: TASK_DAYS }
  }
  return { start: addDays(anchor, index * TASK_DAYS), duration: TASK_DAYS }
}

export function mapProjectToTimeline(project: ProjectPlan): ProjectTimeline {
  const nodes = project.workNodes ?? []
  if (nodes.length === 0) return { tasks: [], links: [], synthetic: false }

  const dated = nodes
    .map((node) => dateOnly(node.dueAt))
    .filter((date): date is Date => date !== null)
  // Any work item without a real due date means part of this schedule is sample
  // data, and the pane says so rather than presenting it as a project fact.
  const synthetic = dated.length < nodes.length
  const anchor =
    dated.length > 0
      ? new Date(Math.min(...dated.map((date) => date.getTime())))
      : SAMPLE_SCHEDULE_START

  // The project itself is the root summary, so the task list carries context.
  // Its progress is REAL (the projection already computed it from persisted WBS).
  //
  // `open: true` belongs ONLY on a task that actually has children. This is not
  // cosmetic: `open` marks a task as EXPANDED, and the Gantt clears an expanded
  // task that has no children to `data: null`. Its own `toArray()` then recurses
  // into `task.data` WITHOUT a null guard and throws
  // "Cannot read properties of null (reading 'forEach')" — which is exactly how
  // this pane first blew up in the browser. Leaf tasks must never be marked open.
  // There is a regression test for this invariant.
  const tasks: ITask[] = [
    {
      id: 1,
      text: project.title,
      type: 'summary',
      parent: 0,
      open: true,
      progress: project.progress,
    },
  ]

  let nextId = 2
  const topLevelIds: number[] = []

  nodes.forEach((node, index) => {
    const id = nextId++
    const { start, duration } = scheduleFor(index, node.dueAt, anchor)
    const isBranch = (node.children?.length ?? 0) > 0
    tasks.push({
      id,
      text: node.title,
      type: isBranch ? 'summary' : 'task',
      parent: 1,
      start,
      duration,
      progress: PROGRESS_BY_STATUS[node.status] ?? 0,
      ...(isBranch ? { open: true } : {}),
      // The inspector's note is real; `details` is the Gantt's own field for it.
      ...(node.note ? { details: node.note } : {}),
    })
    topLevelIds.push(id)

    for (const child of node.children ?? []) {
      const childId = nextId++
      const scheduled = scheduleFor(index, child.dueAt, anchor)
      tasks.push({
        id: childId,
        text: child.title,
        type: 'task',
        parent: id,
        start: scheduled.start,
        duration: scheduled.duration,
        progress: PROGRESS_BY_STATUS[child.status] ?? 0,
        ...(child.note ? { details: child.note } : {}),
      })
    }
  })

  const links: ILink[] = synthetic
    ? topLevelIds.slice(1).map((id, index) => ({
        id: index + 1,
        source: topLevelIds[index],
        target: id,
        type: 'e2s',
      }))
    : []

  return { tasks, links, synthetic }
}
