// ---------------------------------------------------------------------------
// CATCH-UP — supported task taxonomy (workstream -> category).
//
// Single source of truth for the Catch-Up WORKSTREAM / CATEGORY dropdowns in
// the Task Workspace. The canonical columns are task.workstream and
// task.category; this module only defines the SUPPORTED set for the UI so the
// dependent dropdowns can never permit an invalid Workstream/Category pair.
// No new taxonomy model or lookup table is created.
// ---------------------------------------------------------------------------

import { CATCHUP_WORKSTREAMS } from './task-tree'

export { CATCHUP_WORKSTREAMS }

export const CATCHUP_CATEGORIES: Record<string, string[]> = {
  CLIENT: ['FOLLOWUP', 'ONBOARDING', 'CONTRACTS', 'MEDIA'],
  CORE: ['ACCOUNTING', 'MARKETING', 'LEGAL', 'MANAGEMENT'],
  OPPS: ['DATA_ENTRY'],
  SUPPORT: ['SYSTEMS', 'SECURITY'],
  TECH: ['NEW_TECH', 'INFRASTRUCTURE'],
}

/** Valid category choices for a workstream (empty when the workstream is unknown). */
export function categoriesForWorkstream(workstream: string): string[] {
  return CATCHUP_CATEGORIES[workstream] ?? []
}
