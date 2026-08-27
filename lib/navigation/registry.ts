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
  'ACCOUNTING',
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
    label: 'CORE',
    description:
      'Real-estate operating environment — people, properties, deals, showings, offers, documents and activity.',
    home: '/portal/dashboard',
    items: [
      // CORE — the visible primary operating menu. Per the 2026-08-25 CTO/Product
      // Owner decision, Workflows and Forms are visible CORE destinations. This
      // supersedes the earlier ARCH-HANDOFF continuity note ("Workflows and Forms
      // remain functional but hidden from visible CORE navigation"). Both entries
      // reuse the existing /portal/workflows and /portal/forms screens — nothing
      // was rebuilt or re-routed.
      { label: 'Cockpit', href: '/portal/dashboard', authority: 'portal.read' },
      { label: 'Clients', href: '/portal/clients', authority: 'portal.read' },
      { label: 'Catch-Up', href: '/portal/catch-up', authority: 'portal.read' },
      { label: 'Contracts', href: '/portal/deals', authority: 'deal.read' },
      { label: 'Cabinet', href: '/portal/documents', authority: 'deal.read' },
      { label: 'Workflows', href: '/portal/workflows', authority: 'portal.read' },
      { label: 'Forms', href: '/portal/forms', authority: 'deal.read' },
    ],
  },
  ACCOUNTING: {
    surface: 'ACCOUNTING',
    label: 'ACCOUNTING',
    description:
      'Brokerage accounting — receivables, expenses, P&L and receipt intake. A simple cash-oriented management view (not QuickBooks).',
    home: '/portal/accounting',
    items: [
      { label: 'Dashboard', href: '/portal/accounting', authority: 'portal.read' },
      {
        label: 'Receivables',
        href: '/portal/accounting/receivables',
        authority: 'portal.read',
      },
      {
        label: 'Expenses',
        href: '/portal/accounting/expenses',
        authority: 'portal.read',
      },
      {
        label: 'P&L Statement',
        href: '/portal/accounting/pnl',
        authority: 'portal.read',
      },
      {
        label: 'Receipt Scanner',
        href: '/portal/accounting/receipt-scanner',
        authority: 'portal.read',
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
        label: 'Issue Queue',
        href: '/portal/issues',
        authority: 'portal.read',
      },
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
      {
        label: 'Client Administration',
        href: '/portal/client-admin',
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
    // Accessing TECH requires tech.access (ROOT only). Non-tech actors (e.g.
    // BUSINESS_POWER) must not see it in nav and must not open TECH routes.
    accessAuthority: 'tech.access',
    home: '/portal/tech',
    items: [
      {
        label: 'Tech Overview',
        href: '/portal/tech',
        authority: 'tech.access',
      },
      {
        label: 'Command Center',
        href: '/portal/command-center',
        authority: 'tech.access',
      },
      { label: 'Story Board', href: '/portal/storyboard', authority: 'tech.access' },
      {
        label: 'Command Console',
        href: '/portal/command-console',
        authority: 'tech.access',
      },
      { label: 'UI Lab', href: '/portal/design-lab', authority: 'tech.access' },
      { label: 'DB Test', href: '/portal/db-test', authority: 'tech.access' },
      { label: 'Media Test', href: '/portal/media-test', authority: 'tech.access' },
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
