import type { ProjectDomainKey, ProjectWorkNode, ProjectWorkStatus } from "./model"

/** One canonical WBS row projected for the cross-project Catch-Up workbench. */
export type ProjectCatchUpItem = {
  id: string
  projectId: string
  projectTitle: string
  domain: ProjectDomainKey
  contextLabel?: string
  node: ProjectWorkNode
}

export type CatchUpBucket = "today" | "unscheduled"

export type ProjectCatchUpBuckets = {
  today: ProjectCatchUpItem[]
  unscheduled: ProjectCatchUpItem[]
}

export function localDateKey(date: Date): string {
  const year = date.getFullYear()
  const month = String(date.getMonth() + 1).padStart(2, "0")
  const day = String(date.getDate()).padStart(2, "0")
  return `${year}-${month}-${day}`
}

/**
 * WBS due dates are calendar dates, not instants. Preserve the persisted
 * YYYY-MM-DD prefix rather than converting through the browser timezone (which
 * can turn UTC midnight into the previous day in Puerto Rico).
 */
export function dueDateKey(value: string | undefined): string | null {
  if (!value) return null
  const match = /^(\d{4}-\d{2}-\d{2})/.exec(value)
  if (match) return match[1]
  const parsed = new Date(value)
  if (Number.isNaN(parsed.getTime())) return null
  const year = parsed.getUTCFullYear()
  const month = String(parsed.getUTCMonth() + 1).padStart(2, "0")
  const day = String(parsed.getUTCDate()).padStart(2, "0")
  return `${year}-${month}-${day}`
}

const STATUS_RANK: Record<ProjectWorkStatus, number> = {
  blocked: 0,
  "in-progress": 1,
  waiting: 2,
  "not-started": 3,
  complete: 4,
  dismissed: 5,
}

function sortWork(a: ProjectCatchUpItem, b: ProjectCatchUpItem): number {
  const statusOrder = STATUS_RANK[a.node.status] - STATUS_RANK[b.node.status]
  if (statusOrder !== 0) return statusOrder
  const projectOrder = a.projectTitle.localeCompare(b.projectTitle)
  return projectOrder || a.node.title.localeCompare(b.node.title)
}

/**
 * Truthful Catch-Up buckets over canonical WBS rows. No sample rows are ever
 * fabricated here: Today contains real dated rows; Unscheduled contains real
 * unfinished rows whose due date is still null.
 */
export function projectCatchUp(items: readonly ProjectCatchUpItem[], date: Date): ProjectCatchUpBuckets {
  const todayKey = localDateKey(date)
  return {
    today: items
      .filter((item) => item.node.status !== "dismissed" && dueDateKey(item.node.dueAt) === todayKey)
      .slice()
      .sort(sortWork),
    unscheduled: items
      .filter((item) => !item.node.dueAt && item.node.status !== "complete" && item.node.status !== "dismissed")
      .slice()
      .sort(sortWork),
  }
}
