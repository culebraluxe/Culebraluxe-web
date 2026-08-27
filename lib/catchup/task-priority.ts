// ---------------------------------------------------------------------------
// CATCH-UP — task priority mapping (bounded convention).
//
// No canonical priority-label mapping exists anywhere in the task/domain code,
// so this uses the agreed bounded convention for the UI:
//
//   0 = LOW
//   1 = MEDIUM
//   2 = HIGH
//
// The database column stays task.priority (smallint) — no second representation.
// Historical rows are NOT rewritten; the display maps any integer to the nearest
// level, and saving writes the selected level back through the canonical seam.
// ---------------------------------------------------------------------------

export const PRIORITY_LEVELS = [
  { value: 0, label: 'LOW' },
  { value: 1, label: 'MEDIUM' },
  { value: 2, label: 'HIGH' },
] as const

export type PriorityLevel = (typeof PRIORITY_LEVELS)[number]['value']

/** Map a stored integer priority to a display level (0 / 1 / 2). */
export function priorityToLevel(priority: number): PriorityLevel {
  if (priority <= 0) return 0
  if (priority === 1) return 1
  return 2
}

/** Human label for a stored integer priority. */
export function priorityLabel(priority: number): string {
  return PRIORITY_LEVELS[priorityToLevel(priority)].label
}
