// ---------------------------------------------------------------------------
// UI-01 — Operating-surface registry: the single source of truth for
// "which operating surface owns this route" and "what navigation belongs
// under this surface."
//
// Only EXISTING routes are listed — destinations that do not yet exist are
// omitted (future work), never invented to fill slots.
// ---------------------------------------------------------------------------

import type {
  OperatingSurface,
  OperatingSurfaceDefinition,
  SurfaceNavItem,
} from './types'

/** Stable Tier-1 ordering. */
export const OPERATING_SURFACE_ORDER: readonly OperatingSurface[] = [
  'NEXUS',
  'OPS',
  'SUPPORT',
  'TECH',
]

export const OPERATING_SURFACES: Record<
  OperatingSurface,
  OperatingSurfaceDefinition
> = {
  NEXUS: {
    surface: 'NEXUS',
    label: 'NEXUS',
    description:
      'Real-estate operating environment — people, properties, deals, showings, offers, documents and activity.',
    home: '/portal/dashboard',
    items: [
      { label: 'Dashboard', href: '/portal/dashboard', authority: 'portal.read' },
      { label: 'Attention', href: '/portal/attention', authority: 'portal.read' },
      { label: 'Clients', href: '/portal/clients', authority: 'portal.read' },
      { label: 'Deals', href: '/portal/deals', authority: 'deal.read' },
      {
        label: 'Workflows',
        href: '/portal/workflows',
        authority: 'portal.read',
      },
      {
        label: 'Forms',
        href: '/portal/forms',
        authority: 'deal.read',
      },
      {
        label: 'Documents',
        href: '/portal/documents',
        authority: 'deal.read',
      },
    ],
  },
  OPS: {
    surface: 'OPS',
    label: 'OPPS',
    description:
      'Office and business administration — users, property/deal admin, intake resolution, content, data correction and configuration.',
    home: '/portal/needs-review',
    items: [
      {
        label: 'Needs Review',
        href: '/portal/needs-review',
        authority: 'portal.read',
      },
      {
        label: 'Property Admin',
        href: '/portal/property-admin',
        authority: 'portal.read',
      },
      {
        label: 'Media Audit',
        href: '/portal/media-admin',
        authority: 'portal.read',
      },
      {
        // Operations function: matching pictures to properties (media uploader).
        label: 'Property Media',
        href: '/portal/property-media',
        authority: 'portal.read',
      },
      {
        label: 'Identity Quality',
        href: '/portal/identity-quality',
        authority: 'portal.read',
      },
      { label: 'Reporting', href: '/portal/reporting', authority: 'portal.read' },
    ],
  },
  TECH: {
    surface: 'TECH',
    label: 'TECH',
    description:
      'Engineering and platform capability — Forge, Story Board, workflow engineering, MQ/replay, integration checkpoints and engineering evidence.',
    home: '/portal/storyboard',
    items: [
      {
        label: 'Command Center',
        href: '/portal/command-center',
        authority: 'portal.read',
      },
      { label: 'Story Board', href: '/portal/storyboard', authority: 'portal.read' },
      {
        label: 'Command Console',
        href: '/portal/command-console',
        authority: 'portal.read',
      },
      { label: 'DB Test', href: '/portal/db-test', authority: 'portal.read' },
      { label: 'Media Test', href: '/portal/media-test', authority: 'portal.read' },
    ],
  },
  SUPPORT: {
    surface: 'SUPPORT',
    label: 'SUPPORT',
    description:
      'Technology operations — health, incidents, recovery, releases and runtime/environment.',
    home: '/portal/system-health',
    items: [
      {
        label: 'System Health',
        href: '/portal/system-health',
        authority: 'portal.read',
      },
      {
        label: 'Security',
        href: '/portal/settings',
        authority: 'settings.read',
      },
    ],
  },
}

/**
 * Which operating surface owns this route? Longest-prefix match over every
 * surface's items (sub-routes inherit their parent surface, e.g.
 * /portal/command-console/<storyId> → TECH). Unmapped portal routes default
 * to NEXUS — the primary real-estate operating environment.
 */
export function surfaceForPathname(pathname: string): OperatingSurface {
  let best: { surface: OperatingSurface; href: string } | null = null
  for (const surface of OPERATING_SURFACE_ORDER) {
    for (const item of OPERATING_SURFACES[surface].items) {
      if (pathname === item.href || pathname.startsWith(`${item.href}/`)) {
        if (!best || item.href.length > best.href.length) {
          best = { surface, href: item.href }
        }
      }
    }
  }
  return best?.surface ?? 'NEXUS'
}

/** Tier-2 contextual navigation for one operating surface. */
export function navigationForSurface(
  surface: OperatingSurface,
): SurfaceNavItem[] {
  return OPERATING_SURFACES[surface].items
}

/** Landing destination for a selected operating surface. */
export function surfaceHome(surface: OperatingSurface): string {
  return OPERATING_SURFACES[surface].home
}

/** Full definition for one operating surface. */
export function surfaceDefinition(
  surface: OperatingSurface,
): OperatingSurfaceDefinition {
  return OPERATING_SURFACES[surface]
}

/** Strict membership guard for the four canonical surface tokens. */
export function isOperatingSurface(
  value: string | null | undefined,
): value is OperatingSurface {
  return (
    value !== null &&
    value !== undefined &&
    (OPERATING_SURFACE_ORDER as readonly string[]).includes(value)
  )
}
