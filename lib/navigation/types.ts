// ---------------------------------------------------------------------------
// UI-01 — Operating-surface navigation registry: types.
//
// One central navigation registry. Every route in the operating shell belongs
// to exactly one operating surface; every surface owns its Tier-2 contextual
// navigation. "Portal" is no longer the user-facing shell concept.
// ---------------------------------------------------------------------------

import type { AuthorityCode } from '../auth/types'
import type { SecurityLevel } from '@/services/security/level'

/** The stable operating surfaces. */
export type OperatingSurface =
  | 'NEXUS'
  | 'ACCOUNTING'
  | 'MARKETING'
  | 'OPS'
  | 'TECH'
  | 'SUPPORT'

/** One Tier-2 navigation item (an EXISTING route — never invented). */
export type SurfaceNavItem = {
  /** User-facing label (e.g. 'Clients'). */
  label: string
  /** Existing application route (e.g. /portal/clients). */
  href: string
  /**
   * Optional broad SecurityService level for UI visibility. This is deliberately
   * not fine-grained entitlement enforcement.
   */
  minSecurityLevel?: SecurityLevel
  /**
   * Existing entitlement-compatible permission reference. Cosmetic UI gating
   * only for now; server guards remain authoritative.
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
  /** Optional broad SecurityService level required to see the surface. */
  minSecurityLevel?: SecurityLevel
  /**
   * Existing entitlement-compatible permission reference. Kept for the later
   * Entitlement service pass; hiding nav is never security enforcement.
   */
  accessAuthority?: AuthorityCode
  /** Landing destination when the surface is selected (portal.read reachable). */
  home: string
  /** Tier-2 contextual navigation (existing routes only). */
  items: SurfaceNavItem[]
}
