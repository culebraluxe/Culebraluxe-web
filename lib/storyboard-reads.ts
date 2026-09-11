// ---------------------------------------------------------------------------
// Story Board read seam.
//
// WHY THIS EXISTS: the architecture gate forbids components from importing the
// db layer directly (`no-db-import-from-components` in .dependency-cruiser.js:
// "Components must not touch the db layer directly; go through a service/lib
// seam"). This is that seam for the review surfaces, so the component can stay
// presentational and the db import has exactly one owner.
//
// Server-only: callers are React Server Components. Do not import from client
// code.
// ---------------------------------------------------------------------------

export {
  listStoryboardStories,
  listStoryExecutionSummaries,
} from '@/db/storyboard'
