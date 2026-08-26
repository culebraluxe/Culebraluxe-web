// ---------------------------------------------------------------------------
// UI-01 — Operating-surface navigation registry: types.
//
// One central navigation registry. Every route in the operating shell belongs
// to exactly one operating surface; every surface owns its Tier-2 contextual
// navigation. "Portal" is no longer the user-facing shell concept.
// ---------------------------------------------------------------------------

import type { AuthorityCode } from '../auth/types'

/** The four stable operating surfaces. */
export type OperatingSurface = 'NEXUS' | 'OPS' | 'TECH' | 'SUPPORT'

/** One Tier-2 navigation item (an EXISTING route — never invented). */
export type SurfaceNavItem = {
  /** User-facing label (e.g. 'Clients'). */
  label: string
  /** Existing application route (e.g. /portal/clients). */
  href: string
  /**
   * Optional existing permission reference. Cosmetic UI gating only — the
   * security boundary remains the server-side auth guards; navigation
   * visibility is never security enforcement.
   */
  authority?: AuthorityCode
}

/** One operating surface definition. */
export type OperatingSurfaceDefinition = {
  surface: OperatingSurface
  /** Tier-1 label (the surface token). */
  label: string
  /** One-line description of the operating world. */
  description: string
  /**
   * Optional authority required to even SEE the Tier-1 surface capsule (e.g.
   * TECH requires tech.access). Cosmetic UI gating only — the security boundary
   * remains the server-side guards; hiding nav is never security enforcement.
   */
  accessAuthority?: AuthorityCode
  /** Landing destination when the surface is selected (portal.read reachable). */
  home: string
  /** Tier-2 contextual navigation (existing routes only). */
  items: SurfaceNavItem[]
}
