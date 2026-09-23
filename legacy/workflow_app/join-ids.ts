import { isUuidLike } from '@/lib/deal-admin'

// ---------------------------------------------------------------------------
// The join keys an engine read may hand to Postgres as `uuid[]`.
//
// WHY THIS EXISTS, as a bug rather than as a precaution. The workflow engine stores
// `subject_type` / `subject_id` as TEXT — the engine knows nothing about the portal's
// tables, so it accepts any string as a subject. The portal's reads then relate those
// subjects to portal tables with a list cast:
//
//     where d.id = any(${ids}::uuid[])
//
// and Postgres rejects the ENTIRE statement when a single element of that list is not a
// uuid:
//
//     invalid input syntax for type uuid (22P02)
//
// One engine instance recorded with `subject_type = 'deal'` and a non-uuid `subject_id`
// was therefore enough to take down every read that joined on it — the system-health
// diagnostics snapshot, and `getWorkflowSummaries`, which the Workflows screen and the
// Command Center both call. The screen answered 500 and the failure pointed at a cast,
// not at the row that caused it.
//
// NOTHING IS LOST BY DROPPING THEM: an id that is not a uuid cannot name a portal record
// keyed by uuid, so the join would have found no row for it anyway. The workflow keeps a
// null property name, which is what it showed before the join was added.
//
// The validator is the repository's own — `lib/deal-admin`, which has no database imports
// — and the same one the flight recorder uses for engine ids.
// ---------------------------------------------------------------------------

/**
 * The ids from an engine read that CAN be used as `ANY(...::uuid[])`.
 *
 * Blank and non-uuid values are dropped rather than passed on, because one of them fails the whole query. Duplicates are
 * collapsed because the list is only ever a membership test.
 */
export function portalJoinableIds(values: (string | null | undefined)[]): string[] {
  const ids = new Set<string>()
  for (const value of values) {
    if (typeof value !== 'string') continue
    if (!isUuidLike(value)) continue
    ids.add(value)
  }
  return [...ids]
}