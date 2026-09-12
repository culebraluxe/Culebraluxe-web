// ---------------------------------------------------------------------------
// The story priority vocabulary and its ordering ladder — ONE definition.
//
// This was three identical copies (storyboard-data.ts, factory-kpi.ts and
// factory-command-center-data.ts) that drifted apart in exactly the way copies do:
// two of them silently omitted the entries the third had. This module is the
// single source; the three call sites now re-export from here.
//
// 'Reference' is a real member, not a typo of something else. Nine rows on the
// board carry it (ARCH-HANDOFF, SOP1, DEEP1, PORTAL-06, and the five Forge
// ENG-FORGE-HIST-V1..V5 history stories): they are durable handoff/reference
// records, deliberately non-rollup, and they must never create an agent work
// item. Ranking them LAST is what keeps them out of the way of real work.
// ---------------------------------------------------------------------------

export const STORY_PRIORITIES = [
  'Critical',
  'High',
  'High-ish',
  'Medium-High',
  'Medium',
  'Low',
  'Later',
  'High-value polish',
  'Reference',
] as const

export type StoryPriority = (typeof STORY_PRIORITIES)[number]

/**
 * ENG-16 priority ladder — Critical first; unknown priorities rank last.
 * 'Reference' ranks last among KNOWN values (reference rows are not work), and
 * anything genuinely unknown still falls through to the 99 floor below.
 */
const PRIORITY_RANK: Record<string, number> = {
  Critical: 0,
  High: 1,
  'High-ish': 2,
  'Medium-High': 3,
  Medium: 4,
  Low: 5,
  Later: 6,
  'High-value polish': 7,
  Reference: 8,
}

export function priorityRankOf(priority: string): number {
  return PRIORITY_RANK[priority] ?? 99
}
