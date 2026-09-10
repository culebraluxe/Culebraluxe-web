import type { WbsItem } from '@/services/wbs'

export type ProjectCalendarItem = {
  id: string
  title: string
  startAt: string
  status: WbsItem['status']
  category: WbsItem['category']
  owner: string | null
}

/** Project-scoped calendar projection. No due date means no invented event. */
export function mapProjectCalendarItems(items: readonly WbsItem[]): ProjectCalendarItem[] {
  return items
    .filter((item): item is WbsItem & { dueAt: string } => Boolean(item.dueAt && !Number.isNaN(new Date(item.dueAt).getTime())))
    .sort((a, b) => a.dueAt.localeCompare(b.dueAt) || a.id.localeCompare(b.id))
    .map((item) => ({ id: item.id, title: item.title, startAt: item.dueAt, status: item.status, category: item.category, owner: item.owner }))
}
