// ---------------------------------------------------------------------------
// Review dashboard read seam.
//
// WHY THIS EXISTS: the architecture gate forbids components from importing the
// db layer directly (`no-db-import-from-components` in .dependency-cruiser.js:
// "Components must not touch the db layer directly; go through a service/lib
// seam"). The review dashboard is a review-only surface; this seam keeps its
// three reads behind one boundary instead of three component-level db imports.
//
// Server-only: the caller is a React Server Component.
// ---------------------------------------------------------------------------

export { getClients } from '@/db/clients'
export { getDeals } from '@/db/deals'
export { getDashboardSnapshot } from '@/db/dashboard'
